//! Control panel. The page in ui/ draws everything; this module is its
//! controller: it owns the config and the worker threads, turns the page's
//! messages into changes, and sends the page a snapshot of the state
//! (`App::view`) whenever something changes.

mod assets;
mod diag;
mod window;

use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crate::config::{Config, EqMethod, EqMode, Profile};
use crate::protocol::{self, EqPreset, EQ_BANDS};
use crate::winaudio::{self, AudioDevice, Endpoint};
use crate::worker::{self, AudioCmd, AudioState, Change, DevCmd, DevEvent, DeviceInfo, DiagCmd, Notify, Target};
use crate::{connlog, debuglog, registry, synapse};

pub use window::run;

const SAVE_DELAY: Duration = Duration::from_millis(500);
const CONN_LOG_REFRESH: Duration = Duration::from_secs(3);

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
    /// Default device set on connect; `flow` is "out" or "in", "" id = don't change.
    DefaultDevice {
        flow: String,
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
    Autostart {
        on: bool,
    },
    DebugLog {
        on: bool,
    },
    Advanced {
        eq_method: EqMethod,
        release_remote: bool,
        send_legacy_config: bool,
    },
    /// Open something outside the app: "mixer", "sound", "connlog",
    /// "debuglog", "logdir", "configdir".
    Open {
        what: String,
    },
    WizardOpen,
    WizardStart,
    WizardPress {
        k: usize,
    },
    WizardAnswer {
        heard: bool,
    },
    WizardFinish,
    WizardCancel,
}

pub struct App {
    cfg: Config,
    demo: bool,
    dev_tx: Sender<DevCmd>,
    dev_rx: Receiver<DevEvent>,
    audio_tx: Sender<AudioCmd>,
    audio_rx: Receiver<AudioState>,
    info: DeviceInfo,
    audio: AudioState,
    dirty_since: Option<Instant>,
    autostart: bool,
    /// Connection log shown in the Power tab, re-read every few seconds.
    conn_log: Vec<String>,
    conn_drops_today: usize,
    conn_log_read: Instant,
    wizard: Option<diag::Wizard>,
    /// Notifications for the page, taken by the window.
    toasts: Vec<(String, bool)>,
    /// The state changed since the page last got it.
    changed: bool,
}

impl App {
    pub fn new(demo: bool, notify: Notify) -> Self {
        let cfg = Config::load();
        let (dev_tx, dev_rx) = worker::spawn_device_worker(Target::from_config(&cfg), demo, notify.clone());
        let (audio_tx, audio_rx) = worker::spawn_audio_worker(demo, notify);
        Self {
            cfg,
            demo,
            dev_tx,
            dev_rx,
            audio_tx,
            audio_rx,
            info: DeviceInfo::default(),
            audio: AudioState::default(),
            dirty_since: None,
            autostart: registry::autostart_enabled(),
            conn_log: connlog::recent(12),
            conn_drops_today: connlog::drops_today(),
            conn_log_read: Instant::now(),
            wizard: None,
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

    /// Save and push a change to the headset.
    fn push(&mut self, change: Change) {
        self.mark_dirty();
        let _ = self.dev_tx.send(DevCmd::Update(Target::from_config(&self.cfg), change));
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
                    if from_button {
                        self.toast(format!("Preset cambiado desde el headset: {}", preset.label()), false);
                    } else {
                        self.toast(
                            format!(
                                "El headset está en {} y no en {}. Si no usaste su botón EQ, no aceptó el cambio: haz la «Prueba guiada» en AJUSTES.",
                                preset.label(),
                                wanted.label()
                            ),
                            true,
                        );
                    }
                }
                DevEvent::Diag(text) => {
                    if let Some(w) = &mut self.wizard {
                        w.on_result(text);
                    }
                }
            }
        }
        while let Ok(state) = self.audio_rx.try_recv() {
            self.audio = state;
            self.changed = true;
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
            Msg::NewProfile => self.add_profile(Profile { name: "Perfil nuevo".into(), ..Profile::default() }),
            Msg::DuplicateProfile => {
                let mut p = self.profile().clone();
                p.name = format!("{} (copia)", p.name);
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
            Msg::DefaultDevice { flow, id } => {
                match flow.as_str() {
                    "out" => self.cfg.default_speaker = id.clone(),
                    "in" => self.cfg.default_microphone = id.clone(),
                    _ => return,
                }
                self.store();
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
            Msg::Autostart { on } => match registry::set_autostart(on) {
                Ok(()) => self.autostart = on,
                Err(e) => self.toast(e, true),
            },
            Msg::DebugLog { on } => {
                self.cfg.debug_log = on;
                debuglog::set_enabled(on);
                self.store();
            }
            Msg::Advanced { eq_method, release_remote, send_legacy_config } => {
                self.cfg.eq_method = eq_method;
                self.cfg.release_remote = release_remote;
                self.cfg.send_legacy_config = send_legacy_config;
                self.push(Change::All);
            }
            Msg::Open { what } => open(&what),
            Msg::WizardOpen => {
                debuglog::set_enabled(true);
                crate::dlog!("prueba guiada abierta; config: {:?}", Target::from_config(&self.cfg).opts);
                let _ = self.dev_tx.send(DevCmd::Diag(DiagCmd::Begin));
                self.wizard = Some(diag::Wizard::new());
            }
            Msg::WizardStart => {
                if let Some(w) = &mut self.wizard {
                    w.start();
                }
            }
            Msg::WizardPress { k } => {
                let can_send = self.info.status.headset_connected && !self.info.busy;
                if let Some(run) = self.wizard.as_mut().and_then(|w| w.press(k, can_send)) {
                    let _ = self.dev_tx.send(DevCmd::Diag(DiagCmd::Run {
                        action: run.action,
                        method: run.method,
                        release: run.release,
                    }));
                }
            }
            Msg::WizardAnswer { heard } => {
                if let Some(w) = &mut self.wizard {
                    w.answer(heard);
                }
            }
            Msg::WizardFinish => {
                let Some(w) = &self.wizard else { return };
                let outcome = w.outcome();
                for line in &outcome.summary {
                    crate::dlog!("respuesta: {line}");
                }
                if let Some((method, release)) = outcome.method {
                    self.cfg.eq_method = method;
                    self.cfg.release_remote = release;
                }
                if let Some(v) = outcome.eq_status {
                    self.cfg.eq_status = v;
                    self.cfg.send_legacy_config = true;
                }
                self.end_wizard();
            }
            Msg::WizardCancel => {
                if self.wizard.is_some() {
                    crate::dlog!("prueba guiada cancelada");
                    self.end_wizard();
                }
            }
        }
    }

    fn end_wizard(&mut self) {
        self.wizard = None;
        self.save_now();
        let _ = self.dev_tx.send(DevCmd::Diag(DiagCmd::End(Target::from_config(&self.cfg))));
        debuglog::set_enabled(self.cfg.debug_log);
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
                        "Perfil de Synapse importado".to_string()
                    } else {
                        format!("{n} perfiles de Synapse importados")
                    },
                    false,
                );
            }
            Err(e) => self.toast(format!("{name}: {e}"), true),
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
        if !self.info.dongle {
            ("Dongle no encontrado", "error")
        } else if !self.info.status.headset_connected {
            ("Headset apagado o fuera de alcance", "warn")
        } else {
            ("Conectado", "ok")
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
                "product": if self.info.product.is_empty() { "Razer BlackShark V2 Pro" } else { &self.info.product },
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
                "default_out": self.cfg.default_speaker,
                "default_in": self.cfg.default_microphone,
            },
            "settings": {
                "autostart": self.autostart,
                "debug_log": self.cfg.debug_log,
                "eq_method": self.cfg.eq_method,
                "release_remote": self.cfg.release_remote,
                "send_legacy_config": self.cfg.send_legacy_config,
                "eq_status": self.cfg.eq_status,
                "config_path": Config::path().display().to_string(),
            },
            "connlog": { "lines": self.conn_log, "drops_today": self.conn_drops_today },
            "wizard": self.wizard.as_ref().map(|w| w.view(st.headset_connected, self.info.busy)),
        })
    }
}

/// Open a file, folder or Windows panel named by the page.
fn open(what: &str) {
    let dir_of = |path: std::path::PathBuf| {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
            winaudio::open_path(dir);
        }
    };
    match what {
        "mixer" => winaudio::open_volume_mixer(),
        "sound" => winaudio::open_sound_settings(),
        "connlog" => winaudio::open_path(&connlog::path()),
        "debuglog" => winaudio::open_path(&debuglog::path()),
        "logdir" => dir_of(debuglog::path()),
        "configdir" => dir_of(Config::path()),
        _ => {}
    }
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
            parse(r#"{"cmd":"advanced","eq_method":"relatch","release_remote":false,"send_legacy_config":true}"#),
            Msg::Advanced { eq_method: EqMethod::Relatch, release_remote: false, send_legacy_config: true }
        ));
        assert!(serde_json::from_str::<Msg>(r#"{"cmd":"format_disk"}"#).is_err());
    }
}
