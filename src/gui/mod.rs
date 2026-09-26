//! Control panel. The page in ui/ draws everything; this module is its
//! controller: it owns the config and the worker threads, turns the page's
//! messages into changes, and sends the page a snapshot of the state
//! (`App::view`) whenever something changes.

mod assets;
mod window;

use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crate::config::{Config, EqMode, Profile};
use crate::diagnostics::{self, Progress, Report};
use crate::mic::{self, MicEffect, MicEqPreset, MicSettings};
use crate::models::HeadsetModel;
use crate::protocol::{self, EqPreset, EQ_BANDS};
use crate::thx::{self, ThxChange, ThxEq, ThxLevel, ThxOption};
use crate::winaudio::{self, AudioDevice, Endpoint};
use crate::worker::{
    self, AudioCmd, AudioState, Change, DevCmd, DevEvent, DeviceInfo, Notify, Target, ThxCmd, ThxEvent, ThxStatus,
};
use crate::{connlog, debuglog, registry, synapse};

pub use window::run;

const SAVE_DELAY: Duration = Duration::from_millis(500);
const CONN_LOG_REFRESH: Duration = Duration::from_secs(3);
/// The project's home, for help and the source code.
pub const REPO_URL: &str = "https://github.com/Antoniazog493/razer-device-control";
/// Where to report a problem or send a diagnostics report.
pub const ISSUES_URL: &str = "https://github.com/Antoniazog493/razer-device-control/issues/new/choose";
/// How to install THX Spatial Audio without Synapse.
pub const THX_URL: &str = "https://github.com/Antoniazog493/blackshark-v2-pro-thx-restore";

/// A message from the page: `{"cmd": "preset", "preset": "custom"}`.
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Msg {
    /// The page loaded (or reloaded) and wants the full state.
    Ready,
    SelectProfile {
        index: usize,
    },
    NewProfile,
    DuplicateProfile,
    RenameProfile {
        name: String,
    },
    DeleteProfile,
    /// A .synapse4 file the user picked or dropped, read by the page.
    Import {
        name: String,
        text: String,
    },
    ApplyNow,
    EqMode {
        mode: EqMode,
    },
    Preset {
        preset: EqPreset,
    },
    EqReset,
    EqCopyToCustom,
    /// Custom curve after a drag on the graph ends.
    EqBands {
        bands: Vec<i8>,
    },
    /// Windows volume, 0-100.
    Volume {
        id: String,
        value: f32,
    },
    Mute {
        id: String,
        muted: bool,
    },
    /// Make this endpoint the Windows default now (only when picked).
    DefaultDevice {
        id: String,
    },
    Dnd {
        on: bool,
    },
    Sidetone {
        on: bool,
    },
    /// `commit` = the slider was released (send it), else just store it.
    SidetoneVolume {
        value: u8,
        commit: bool,
    },
    AutoOff {
        on: bool,
    },
    AutoOffMinutes {
        value: u8,
        commit: bool,
    },
    /// THX options (Enhancements tab); they live in Windows, not in the profile.
    ThxSpatial {
        on: bool,
    },
    ThxBassBoost {
        on: bool,
    },
    ThxNormalization {
        on: bool,
    },
    ThxVoiceClarity {
        on: bool,
    },
    /// THX levels, 0-100, sent when the slider is released.
    ThxBassBoostLevel {
        value: f32,
    },
    ThxNormalizationLevel {
        value: f32,
    },
    ThxVoiceClarityLevel {
        value: f32,
    },
    /// Microphone enhancements (Microphone tab), stored in the profile.
    MicEqPreset {
        preset: MicEqPreset,
    },
    /// Custom microphone curve after a drag on its graph ends.
    MicEqBands {
        bands: Vec<i8>,
    },
    MicEqReset,
    MicEffect {
        effect: MicEffect,
        on: bool,
    },
    /// Sent when the slider is released.
    MicEffectLevel {
        effect: MicEffect,
        value: i16,
    },
    Autostart {
        on: bool,
    },
    DebugLog {
        on: bool,
    },
    /// The user's headset model (Settings).
    HeadsetModel {
        model: HeadsetModel,
    },
    /// Read-only diagnostics for that model (Settings).
    RunDiagnostics,
    /// Open something outside the app: "mixer", "sound", "connlog",
    /// "debuglog", "logdir", "configdir", "report", "reportdir", "issues",
    /// "repo", "thx".
    Open {
        what: String,
    },
}

/// The diagnostics run shown in Settings.
#[derive(Default)]
struct DiagState {
    running: bool,
    step: String,
    result: Option<Result<Report, String>>,
}

enum DiagEvent {
    Progress(Progress),
    Done(Result<Report, String>),
}

pub struct App {
    cfg: Config,
    demo: bool,
    dev_tx: Sender<DevCmd>,
    dev_rx: Receiver<DevEvent>,
    audio_tx: Sender<AudioCmd>,
    audio_rx: Receiver<AudioState>,
    thx_tx: Sender<ThxCmd>,
    thx_rx: Receiver<ThxEvent>,
    info: DeviceInfo,
    audio: AudioState,
    thx: ThxStatus,
    dirty_since: Option<Instant>,
    autostart: bool,
    /// Connection log shown in the Power tab, re-read every few seconds.
    conn_log: Vec<String>,
    conn_drops_today: usize,
    conn_log_read: Instant,
    diag: DiagState,
    diag_rx: Option<Receiver<DiagEvent>>,
    notify: Notify,
    /// Notifications for the page, taken by the window.
    toasts: Vec<(String, bool)>,
    /// The state changed since the page last got it.
    changed: bool,
}

impl App {
    pub fn new(demo: bool, notify: Notify) -> Self {
        let cfg = Config::load();
        let (dev_tx, dev_rx) = worker::spawn_device_worker(Target::from_config(&cfg), demo, notify.clone());
        let (audio_tx, audio_rx) = worker::spawn_audio_worker(demo, notify.clone());
        let (thx_tx, thx_rx) = worker::spawn_thx_worker(demo, notify.clone());
        // THX may have lost them (a restart); its thread sends them once it can.
        let _ = thx_tx.send(ThxCmd::Mic(cfg.profile().mic));
        Self {
            cfg,
            demo,
            dev_tx,
            dev_rx,
            audio_tx,
            audio_rx,
            thx_tx,
            thx_rx,
            info: DeviceInfo::default(),
            audio: AudioState::default(),
            thx: ThxStatus::default(),
            dirty_since: None,
            autostart: registry::autostart_enabled(),
            conn_log: connlog::recent(12),
            conn_drops_today: connlog::drops_today(),
            conn_log_read: Instant::now(),
            diag: DiagState::default(),
            diag_rx: None,
            notify,
            toasts: Vec::new(),
            changed: true,
        }
    }

    fn profile(&self) -> &Profile {
        self.cfg.profile()
    }

    fn profile_mut(&mut self) -> &mut Profile {
        self.cfg.profile_mut()
    }

    fn mark_dirty(&mut self) {
        self.dirty_since.get_or_insert_with(Instant::now);
        self.changed = true;
    }

    /// Save and push a change to the headset (and its EQ to THX).
    fn push(&mut self, change: Change) {
        self.mark_dirty();
        let _ = self.dev_tx.send(DevCmd::Update(Target::from_config(&self.cfg), change));
        if matches!(change, Change::All | Change::Eq) {
            self.sync_thx_eq();
        }
        if change == Change::All {
            self.sync_thx_mic();
        }
    }

    /// The microphone enhancements go to THX, which forgets them when its
    /// service restarts; its thread sends them again then (ADR 0007).
    fn sync_thx_mic(&mut self) {
        let _ = self.thx_tx.send(ThxCmd::Mic(self.profile().mic));
    }

    /// A microphone change: save it and send it to THX (and the EQ preset's
    /// index to the headset, like Synapse).
    fn set_mic(&mut self, f: impl FnOnce(&mut mic::MicSettings)) {
        let before = self.profile().mic;
        let m = &mut self.profile_mut().mic;
        f(m);
        m.sanitize();
        let after = *m;
        if after == before {
            return;
        }
        if after.eq_preset != before.eq_preset {
            self.push(Change::MicEq);
        } else {
            self.store();
        }
        self.sync_thx_mic();
    }

    /// Like Synapse, the headset preset also picks THX's preset and curve,
    /// which is the EQ that is heard (ADR 0006).
    fn sync_thx_eq(&mut self) {
        if self.thx.service {
            let p = self.profile();
            let _ = self.thx_tx.send(ThxCmd::Eq(ThxEq::new(p.active_preset(), p.active_curve())));
        }
    }

    /// Save a change the headset doesn't need right now.
    fn store(&mut self) {
        self.mark_dirty();
        let _ = self.dev_tx.send(DevCmd::SetTarget(Target::from_config(&self.cfg)));
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push((text.into(), error));
    }

    pub fn take_toasts(&mut self) -> Vec<(String, bool)> {
        std::mem::take(&mut self.toasts)
    }

    /// The state to send, if it changed since last time.
    pub fn take_view(&mut self) -> Option<Value> {
        std::mem::take(&mut self.changed).then(|| self.view())
    }

    pub fn save_now(&mut self) {
        if let Err(e) = self.cfg.save() {
            self.toast(e, true);
        }
        self.dirty_since = None;
    }

    /// Before the window closes.
    pub fn shutdown(&mut self) {
        if self.dirty_since.is_some() {
            let _ = self.cfg.save();
        }
    }

    /// Drain worker events.
    pub fn pump(&mut self) {
        while let Ok(ev) = self.dev_rx.try_recv() {
            self.changed = true;
            match ev {
                DevEvent::Info(info) => self.info = info,
                DevEvent::Message { text, error } => self.toast(text, error),
                DevEvent::PresetChanged { preset, from_button } => {
                    let wanted = self.profile().active_preset();
                    self.profile_mut().select_preset(preset);
                    self.mark_dirty();
                    self.sync_thx_eq();
                    if from_button {
                        self.toast(format!("Preset changed on the headset: {}", preset.label()), false);
                    } else {
                        self.toast(
                            format!(
                                "The headset is on {} instead of {}: it did not take the change. Try again, and turn on the debug log in Settings if it keeps happening.",
                                preset.label(),
                                wanted.label()
                            ),
                            true,
                        );
                    }
                }
            }
        }
        while let Some(ev) = self.diag_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.changed = true;
            match ev {
                DiagEvent::Progress(Progress::Step(step)) => self.diag.step = step.into(),
                DiagEvent::Progress(Progress::Listening { left, reports }) => {
                    self.diag.step = format!(
                        "Listening for {left} s: turn the headset off and on, press its buttons, turn its dials… ({reports} report{} so far)",
                        if reports == 1 { "" } else { "s" }
                    );
                }
                DiagEvent::Done(result) => {
                    if let Err(e) = &result {
                        self.toast(format!("Diagnostics failed: {e}"), true);
                    }
                    self.diag = DiagState { result: Some(result), ..DiagState::default() };
                    self.diag_rx = None;
                }
            }
        }
        while let Ok(state) = self.audio_rx.try_recv() {
            self.audio = state;
            self.changed = true;
        }
        while let Ok(ev) = self.thx_rx.try_recv() {
            self.changed = true;
            match ev {
                ThxEvent::Status(status) => self.thx = status,
                ThxEvent::Message { text, error } => self.toast(text, error),
            }
        }
    }

    /// Timed work: the delayed save and the connection log refresh.
    pub fn tick(&mut self, now: Instant) {
        if self.dirty_since.is_some_and(|t| now >= t + SAVE_DELAY) {
            self.save_now();
        }
        if now >= self.conn_log_read + CONN_LOG_REFRESH {
            self.conn_log_read = now;
            let (log, drops) = (connlog::recent(12), connlog::drops_today());
            if log != self.conn_log || drops != self.conn_drops_today {
                self.conn_log = log;
                self.conn_drops_today = drops;
                self.changed = true;
            }
        }
    }

    /// When `tick` has something to do next.
    pub fn next_tick(&self) -> Instant {
        let log = self.conn_log_read + CONN_LOG_REFRESH;
        self.dirty_since.map_or(log, |t| log.min(t + SAVE_DELAY))
    }

    // --------------------------------------------------------------- messages

    pub fn handle(&mut self, msg: Msg) {
        self.changed = true;
        match msg {
            Msg::Ready => {}
            Msg::SelectProfile { index } => {
                if index != self.cfg.active && index < self.cfg.profiles.len() {
                    self.cfg.active = index;
                    self.push(Change::All);
                }
            }
            Msg::NewProfile => self.add_profile(Profile { name: "New profile".into(), ..Profile::default() }),
            Msg::DuplicateProfile => {
                let mut p = self.profile().clone();
                p.name = format!("{} (copy)", p.name);
                self.add_profile(p);
            }
            Msg::RenameProfile { name } => {
                let name = name.trim().to_string();
                if !name.is_empty() {
                    self.profile_mut().name = name;
                    self.store();
                }
            }
            Msg::DeleteProfile => {
                if self.cfg.profiles.len() > 1 {
                    self.cfg.profiles.remove(self.cfg.active);
                    self.cfg.active = self.cfg.active.saturating_sub(1);
                    self.push(Change::All);
                }
            }
            Msg::Import { name, text } => self.import(&name, &text),
            Msg::ApplyNow => self.push(Change::All),
            Msg::EqMode { mode } => {
                if self.profile().eq_mode != mode {
                    self.profile_mut().eq_mode = mode;
                    self.push(Change::Eq);
                }
            }
            Msg::Preset { preset } => {
                if preset != self.profile().active_preset() {
                    self.profile_mut().select_preset(preset);
                    self.push(Change::Eq);
                }
            }
            Msg::EqReset => {
                self.profile_mut().custom_eq = [0; EQ_BANDS];
                self.push(Change::Eq);
            }
            Msg::EqCopyToCustom => {
                if let Some(curve) = self.profile().active_preset().curve() {
                    let p = self.profile_mut();
                    p.custom_eq = curve;
                    p.select_preset(EqPreset::Custom);
                    self.push(Change::Eq);
                }
            }
            Msg::EqBands { bands } => {
                // Only Custom is editable; the stored presets live in the headset.
                if let (Ok(bands), EqPreset::Custom) =
                    (<[i8; EQ_BANDS]>::try_from(bands), self.profile().active_preset())
                {
                    let bands = bands.map(|b| b.clamp(protocol::EQ_MIN_DB, protocol::EQ_MAX_DB));
                    if bands != self.profile().custom_eq {
                        self.profile_mut().custom_eq = bands;
                        self.push(Change::Eq);
                    }
                }
            }
            Msg::Volume { id, value } => {
                let v = (value / 100.0).clamp(0.0, 1.0);
                self.endpoint_mut(&id, |e| e.volume = v);
                let _ = self.audio_tx.send(AudioCmd::Volume(id, v));
            }
            Msg::Mute { id, muted } => {
                self.endpoint_mut(&id, |e| e.muted = muted);
                let _ = self.audio_tx.send(AudioCmd::Mute(id, muted));
            }
            Msg::DefaultDevice { id } => {
                if !id.is_empty() {
                    let _ = self.audio_tx.send(AudioCmd::DefaultDevice(id));
                }
            }
            Msg::Dnd { on } => {
                self.profile_mut().dnd = on;
                self.push(Change::Dnd);
            }
            Msg::Sidetone { on } => {
                self.profile_mut().sidetone_enabled = on;
                self.push(Change::Sidetone);
            }
            Msg::SidetoneVolume { value, commit } => {
                self.profile_mut().sidetone_volume = value.clamp(1, 100);
                self.mark_dirty();
                if commit {
                    if self.profile().sidetone_enabled {
                        self.push(Change::Sidetone);
                    } else {
                        self.store();
                    }
                }
            }
            Msg::AutoOff { on } => {
                self.profile_mut().auto_off_enabled = on;
                self.push(Change::AutoOff);
            }
            Msg::AutoOffMinutes { value, commit } => {
                self.profile_mut().auto_off_minutes =
                    value.clamp(protocol::AUTO_OFF_MIN_MINUTES, protocol::AUTO_OFF_MAX_MINUTES);
                self.mark_dirty();
                if commit {
                    if self.profile().auto_off_enabled {
                        self.push(Change::AutoOff);
                    } else {
                        self.store();
                    }
                }
            }
            Msg::ThxSpatial { on } => self.set_thx(ThxChange::Switch(ThxOption::Spatial, on)),
            Msg::ThxBassBoost { on } => self.set_thx(ThxChange::Switch(ThxOption::BassBoost, on)),
            Msg::ThxNormalization { on } => self.set_thx(ThxChange::Switch(ThxOption::Normalization, on)),
            Msg::ThxVoiceClarity { on } => self.set_thx(ThxChange::Switch(ThxOption::VoiceClarity, on)),
            Msg::ThxBassBoostLevel { value } => self.set_thx_level(ThxLevel::BassBoost, value),
            Msg::ThxNormalizationLevel { value } => self.set_thx_level(ThxLevel::Normalization, value),
            Msg::ThxVoiceClarityLevel { value } => self.set_thx_level(ThxLevel::VoiceClarity, value),
            Msg::MicEqPreset { preset } => self.set_mic(|m| m.eq_preset = preset),
            Msg::MicEqBands { bands } => {
                if let Ok(bands) = <[i8; EQ_BANDS]>::try_from(bands) {
                    self.set_mic(|m| {
                        if m.eq_preset == MicEqPreset::Custom {
                            m.custom_eq = bands;
                        }
                    });
                }
            }
            Msg::MicEqReset => self.set_mic(|m| m.custom_eq = [0; EQ_BANDS]),
            Msg::MicEffect { effect, on } => self.set_mic(|m| m.effect_mut(effect).on = on),
            Msg::MicEffectLevel { effect, value } => self.set_mic(|m| m.effect_mut(effect).level = value),
            Msg::Autostart { on } => match registry::set_autostart(on) {
                Ok(()) => self.autostart = on,
                Err(e) => self.toast(e, true),
            },
            Msg::DebugLog { on } => {
                self.cfg.debug_log = on;
                debuglog::set_enabled(on);
                self.store();
            }
            Msg::Open { what } => self.open(&what),
            Msg::HeadsetModel { model } => {
                if model != self.cfg.headset_model {
                    self.cfg.headset_model = model;
                    self.push(Change::All);
                }
            }
            Msg::RunDiagnostics => self.run_diagnostics(),
        }
    }

    /// Start the read-only diagnostics for the chosen model in the
    /// background; its progress comes back through `pump`.
    fn run_diagnostics(&mut self) {
        if self.diag.running {
            return;
        }
        let model = self.cfg.headset_model;
        self.diag = DiagState { running: true, step: "Starting…".into(), ..DiagState::default() };
        let (tx, rx) = std::sync::mpsc::channel();
        self.diag_rx = Some(rx);
        let notify = self.notify.clone();
        let demo = self.demo;
        std::thread::spawn(move || {
            let send = |ev: DiagEvent| {
                let _ = tx.send(ev);
                notify();
            };
            let result = if demo {
                demo_diagnostics(&|p| send(DiagEvent::Progress(p)))
            } else {
                diagnostics::run(model, diagnostics::LISTEN, &|p| send(DiagEvent::Progress(p)))
            };
            send(DiagEvent::Done(result));
        });
    }

    fn add_profile(&mut self, mut profile: Profile) {
        profile.name = self.cfg.unique_name(&profile.name);
        self.cfg.profiles.push(profile);
        self.cfg.active = self.cfg.profiles.len() - 1;
        self.push(Change::All);
    }

    fn import(&mut self, name: &str, text: &str) {
        match synapse::import_str(text) {
            Ok(profiles) => {
                let n = profiles.len();
                let first = self.cfg.profiles.len();
                for mut p in profiles {
                    p.name = self.cfg.unique_name(&p.name);
                    self.cfg.profiles.push(p);
                }
                self.cfg.active = first;
                self.push(Change::All);
                self.toast(
                    if n == 1 {
                        "Synapse profile imported".to_string()
                    } else {
                        format!("{n} Synapse profiles imported")
                    },
                    false,
                );
            }
            Err(e) => self.toast(format!("{name}: {e}"), true),
        }
    }

    fn set_thx(&mut self, change: ThxChange) {
        let Some(state) = &mut self.thx.state else { return };
        if !self.thx.service || self.thx.busy || change.shown_in(state) {
            return;
        }
        // Show it now; the THX thread reports back once the service confirms (or not).
        change.apply(state);
        self.thx.busy = true;
        let _ = self.thx_tx.send(ThxCmd::Change(change));
    }

    fn set_thx_level(&mut self, level: ThxLevel, value: f32) {
        if value.is_finite() {
            self.set_thx(ThxChange::Level(level, value.round().clamp(0.0, 100.0)));
        }
    }

    /// Update the local copy of an endpoint so the page doesn't jump back
    /// before the audio worker reports the new state.
    fn endpoint_mut(&mut self, id: &str, f: impl Fn(&mut Endpoint)) {
        for e in [&mut self.audio.headset_out, &mut self.audio.headset_in].into_iter().flatten() {
            if e.id == id {
                f(e);
            }
        }
    }

    // ------------------------------------------------------------------ view

    /// (text, level) for the connection indicator; level is ok/warn/error.
    fn connection(&self) -> (&'static str, &'static str) {
        if !self.cfg.headset_model.supported() {
            ("Model not supported yet", "warn")
        } else if !self.info.dongle {
            ("Dongle not found", "error")
        } else if !self.info.status.headset_connected {
            ("Headset off or out of range", "warn")
        } else {
            ("Connected", "ok")
        }
    }

    /// Everything the page draws.
    pub fn view(&self) -> Value {
        let p = self.profile();
        let st = &self.info.status;
        let (conn_text, conn_level) = self.connection();
        let presets = |list: &[EqPreset]| -> Vec<Value> {
            list.iter().map(|&x| json!({ "id": x, "label": x.label() })).collect()
        };
        let model = self.cfg.headset_model;
        let endpoint = |e: &Option<Endpoint>| {
            e.as_ref()
                .map(|e| json!({ "id": e.id, "name": e.name, "volume": (e.volume * 100.0).round(), "muted": e.muted }))
        };
        let devices = |list: &[AudioDevice]| -> Vec<Value> {
            list.iter().map(|d| json!({ "id": d.id, "name": d.name, "default": d.is_default })).collect()
        };
        let active = p.active_preset();
        json!({
            "demo": self.demo,
            "version": env!("CARGO_PKG_VERSION"),
            "device": {
                "product": if self.info.product.is_empty() || !model.supported() { model.name() } else { &self.info.product },
                "dongle": self.info.dongle,
                "headset": st.headset_connected,
                "busy": self.info.busy,
                "text": conn_text,
                "level": conn_level,
                "battery": st.battery,
                "charging": st.charging,
                "mic_muted": st.mic_muted,
                "firmware": self.info.firmware,
                "dongle_firmware": self.info.dongle_firmware,
                "serial": self.info.serial,
                "preset": st.preset.map(|x| x.label()),
            },
            "model": {
                "id": model,
                "name": model.name(),
                "supported": model.supported(),
                "options": HeadsetModel::ALL.iter().map(|&m| json!({
                    "id": m,
                    "name": m.name(),
                    "hint": m.hint(),
                    "supported": m.supported(),
                })).collect::<Vec<_>>(),
            },
            "diag": {
                "running": self.diag.running,
                "step": self.diag.step,
                "summary": self.diag.result.as_ref().and_then(|r| r.as_ref().ok()).map(|r| &r.summary),
                "file": self.diag.result.as_ref().and_then(|r| r.as_ref().ok()).map(|r| {
                    r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                }),
                "error": self.diag.result.as_ref().and_then(|r| r.as_ref().err()),
            },
            "profiles": self.cfg.profiles.iter().map(|x| &x.name).collect::<Vec<_>>(),
            "active": self.cfg.active,
            "profile": {
                "name": p.name,
                "eq_mode": p.eq_mode,
                "preset": active,
                "editable": active == EqPreset::Custom,
                "has_curve": active.curve().is_some(),
                "curve": p.active_curve(),
                "sidetone_enabled": p.sidetone_enabled,
                "sidetone_volume": p.sidetone_volume,
                "dnd": p.dnd,
                "auto_off_enabled": p.auto_off_enabled,
                "auto_off_minutes": p.auto_off_minutes,
            },
            "presets": { "standard": presets(&EqPreset::STANDARD), "esports": presets(&EqPreset::ESPORTS) },
            "eq": {
                "freqs": protocol::EQ_FREQS,
                "min": protocol::EQ_MIN_DB,
                "max": protocol::EQ_MAX_DB,
            },
            "auto_off_range": [protocol::AUTO_OFF_MIN_MINUTES, protocol::AUTO_OFF_MAX_MINUTES],
            "audio": {
                "out": endpoint(&self.audio.headset_out),
                "in": endpoint(&self.audio.headset_in),
                "outputs": devices(&self.audio.outputs),
                "inputs": devices(&self.audio.inputs),
            },
            "settings": {
                "autostart": self.autostart,
                "debug_log": self.cfg.debug_log,
                "config_path": Config::path().display().to_string(),
            },
            "thx": self.thx.state.as_ref().map(|s| json!({
                "service": self.thx.service,
                "writable": self.thx.service && !self.thx.busy,
                "busy": self.thx.busy,
                "preset": thx::preset_label(&s.preset_name),
                "spatial": s.spatial_enabled,
                "bass_boost": s.bass_boost_enabled,
                "bass_boost_level": s.bass_boost.round(),
                "normalization": s.drc_enabled,
                "normalization_level": s.drc_level.round(),
                "voice_clarity": s.dialog_enhancement_enabled,
                "voice_clarity_level": s.dialog_enhancement.round(),
            })),
            "mic": {
                "eq_preset": p.mic.eq_preset,
                "presets": MicEqPreset::ALL.iter().map(|&x| json!({ "id": x, "label": x.label() })).collect::<Vec<_>>(),
                "editable": p.mic.eq_preset == MicEqPreset::Custom,
                "curve": p.mic.curve(),
                "min": mic::MIC_EQ_MIN_DB,
                "max": mic::MIC_EQ_MAX_DB,
                "effects": MicSettings::EFFECTS.iter().map(|&e| {
                    let fx = p.mic.effect(e);
                    let (min, max) = e.range();
                    (e.id().to_string(), json!({ "on": fx.on, "level": fx.level, "min": min, "max": max }))
                }).collect::<serde_json::Map<_, _>>(),
                // The enhancements need THX's microphone effect and its service.
                "available": self.thx.service,
            },
            "connlog": { "lines": self.conn_log, "drops_today": self.conn_drops_today },
        })
    }

    /// Open a file, folder, web page or Windows panel named by the page.
    fn open(&self, what: &str) {
        let dir_of = |path: std::path::PathBuf| {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
                winaudio::open_path(dir);
            }
        };
        let report = self.diag.result.as_ref().and_then(|r| r.as_ref().ok()).map(|r| r.path.clone());
        match what {
            "mixer" => winaudio::open_volume_mixer(),
            "sound" => winaudio::open_sound_settings(),
            "connlog" => winaudio::open_path(&connlog::path()),
            "debuglog" => winaudio::open_path(&debuglog::path()),
            "logdir" => dir_of(debuglog::path()),
            "configdir" => dir_of(Config::path()),
            "report" => {
                if let Some(path) = report {
                    winaudio::open_path(&path);
                }
            }
            "reportdir" => {
                let dir = diagnostics::dir();
                let _ = std::fs::create_dir_all(&dir);
                winaudio::open_path(&dir);
            }
            "issues" => winaudio::open_path(std::path::Path::new(ISSUES_URL)),
            "repo" => winaudio::open_path(std::path::Path::new(REPO_URL)),
            "thx" => winaudio::open_path(std::path::Path::new(THX_URL)),
            _ => {}
        }
    }
}

/// Fake diagnostics for `--demo` and the page's demo: nothing is read.
fn demo_diagnostics(progress: &dyn Fn(Progress)) -> Result<Report, String> {
    progress(Progress::Step("Looking for Razer devices…"));
    for left in (1..=3).rev() {
        progress(Progress::Listening { left, reports: 3 - left as usize });
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(Report {
        path: diagnostics::dir().join("rzr-diagnostics-demo.txt"),
        summary: vec!["Demo: nothing was read.".into()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Msg {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn page_messages_parse() {
        assert!(matches!(parse(r#"{"cmd":"ready"}"#), Msg::Ready));
        assert!(matches!(
            parse(r#"{"cmd":"preset","preset":"apex_legends"}"#),
            Msg::Preset { preset: EqPreset::ApexLegends }
        ));
        assert!(matches!(parse(r#"{"cmd":"eq_mode","mode":"esports"}"#), Msg::EqMode { mode: EqMode::Esports }));
        assert!(matches!(
            parse(r#"{"cmd":"sidetone_volume","value":40,"commit":true}"#),
            Msg::SidetoneVolume { value: 40, commit: true }
        ));
        assert!(matches!(
            parse(r#"{"cmd":"headset_model","model":"blackshark_v2_pro_2020"}"#),
            Msg::HeadsetModel { model: HeadsetModel::BlackSharkV2Pro2020 }
        ));
        assert!(matches!(parse(r#"{"cmd":"run_diagnostics"}"#), Msg::RunDiagnostics));
        assert!(matches!(parse(r#"{"cmd":"thx_bass_boost","on":false}"#), Msg::ThxBassBoost { on: false }));
        assert!(matches!(parse(r#"{"cmd":"thx_voice_clarity","on":true}"#), Msg::ThxVoiceClarity { on: true }));
        assert!(matches!(parse(r#"{"cmd":"thx_spatial","on":true}"#), Msg::ThxSpatial { on: true }));
        assert!(matches!(parse(r#"{"cmd":"thx_normalization","on":true}"#), Msg::ThxNormalization { on: true }));
        // Sliders also send `commit`, which THX levels don't need.
        assert!(matches!(
            parse(r#"{"cmd":"thx_bass_boost_level","value":35,"commit":true}"#),
            Msg::ThxBassBoostLevel { value } if value == 35.0
        ));
        assert!(matches!(
            parse(r#"{"cmd":"thx_normalization_level","value":0,"commit":true}"#),
            Msg::ThxNormalizationLevel { value } if value == 0.0
        ));
        assert!(matches!(
            parse(r#"{"cmd":"thx_voice_clarity_level","value":100,"commit":true}"#),
            Msg::ThxVoiceClarityLevel { value } if value == 100.0
        ));
        assert!(matches!(
            parse(r#"{"cmd":"mic_eq_preset","preset":"mic_boost"}"#),
            Msg::MicEqPreset { preset: MicEqPreset::MicBoost }
        ));
        assert!(matches!(
            parse(r#"{"cmd":"mic_effect","effect":"voice_gate","on":true}"#),
            Msg::MicEffect { effect: MicEffect::VoiceGate, on: true }
        ));
        assert!(matches!(
            parse(r#"{"cmd":"mic_effect_level","effect":"noise_reduction","value":80,"commit":true}"#),
            Msg::MicEffectLevel { effect: MicEffect::NoiseReduction, value: 80 }
        ));
        assert!(serde_json::from_str::<Msg>(r#"{"cmd":"format_disk"}"#).is_err());
        // The guided EQ test is gone.
        assert!(serde_json::from_str::<Msg>(r#"{"cmd":"wizard_open"}"#).is_err());
    }
}
