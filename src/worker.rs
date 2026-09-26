//! Background threads for the GUI: one owns the HID device (writes can take
//! a second, and the link needs polling), one talks to Windows audio, and one
//! to THX (confirming a THX change takes seconds).

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{Config, EqMethod, Profile};
use crate::connlog::ConnLog;
use crate::device::{self, Device, Options, Status};
use crate::dlog;
use crate::mic::MicSettings;
use crate::protocol::{EqPreset, EQ_BANDS};
use crate::thx::{self, ThxChange, ThxEq, ThxOption, ThxState};
use crate::winaudio::{self, AudioDevice, Endpoint, Flow};

const POLL_CONNECTED: Duration = Duration::from_secs(5);
const POLL_DISCONNECTED: Duration = Duration::from_secs(3);
/// How often to look for unsolicited link events between polls.
const EVENT_TICK: Duration = Duration::from_millis(250);
/// Without an EQ-button event, ignore a preset mismatch this long after our
/// own writes: a cross-family select can briefly land on the wrong preset.
const PRESET_SETTLE: Duration = Duration::from_secs(4);

/// Called from a worker thread after it sends something, to wake the UI.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// What part of the profile to push to the headset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    All,
    Eq,
    Sidetone,
    Dnd,
    AutoOff,
    MicEq,
}

/// Settings the device worker needs besides the profile.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub profile: Profile,
    pub opts: Options,
}

impl Target {
    pub fn from_config(cfg: &Config) -> Self {
        Self { profile: cfg.profile().clone(), opts: Options::from_config(cfg) }
    }
}

pub enum DevCmd {
    /// New target state; push `change` to the headset now.
    Update(Target, Change),
    /// Store the new target without pushing (applied on next connect).
    SetTarget(Target),
    /// Guided EQ test (AJUSTES).
    Diag(DiagCmd),
}

pub enum DiagCmd {
    /// Stop polling and keep other rzr processes off the dongle.
    Begin,
    /// Send one test setting with the given method, then read the headset
    /// back (answered with DevEvent::Diag).
    Run { action: DiagAction, method: EqMethod, release: bool },
    /// Resume normal operation with this target, re-applying the profile.
    End(Target),
}

#[derive(Clone, Copy, Debug)]
pub enum DiagAction {
    /// Custom preset with this curve, then leave the preset and come back.
    CurveRelatch([i8; EQ_BANDS]),
    /// This curve written into a given preset's slot (e.g. an esports one).
    SlotCurve(EqPreset, [i8; EQ_BANDS]),
    /// Select a preset without writing any curve.
    Select(EqPreset),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviceInfo {
    pub dongle: bool,
    pub product: String,
    pub status: Status,
    pub firmware: Option<String>,
    pub dongle_firmware: Option<String>,
    pub serial: Option<String>,
    pub busy: bool,
}

pub enum DevEvent {
    Info(DeviceInfo),
    Message {
        text: String,
        error: bool,
    },
    /// The headset is playing a different preset than the profile says:
    /// its EQ button switched it, or our write didn't take.
    PresetChanged {
        preset: EqPreset,
        from_button: bool,
    },
    /// Result of a guided-test step: what the headset reports afterwards.
    Diag(String),
}

pub fn spawn_device_worker(target: Target, demo: bool, notify: Notify) -> (Sender<DevCmd>, Receiver<DevEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut w = DevWorker {
            dev: None,
            info: DeviceInfo::default(),
            target,
            tx: ev_tx,
            notify,
            demo,
            last_write: Instant::now() - PRESET_SETTLE,
            button_preset: false,
            first_poll: true,
            diag: false,
            misses: 0,
            log: ConnLog::new("panel"),
        };
        w.run(cmd_rx);
    });
    (cmd_tx, ev_rx)
}

struct DevWorker {
    dev: Option<Device>,
    info: DeviceInfo,
    target: Target,
    tx: Sender<DevEvent>,
    notify: Notify,
    demo: bool,
    last_write: Instant,
    /// The headset sent a preset event since the last poll (its EQ button,
    /// or a select of ours that briefly landed elsewhere).
    button_preset: bool,
    /// The panel just opened: nothing read from the headset yet.
    first_poll: bool,
    /// Guided test running: no polling, no automatic writes.
    diag: bool,
    /// Consecutive polls without the headset; one dropped reply isn't a
    /// disconnect (and would otherwise re-apply the profile on the next poll).
    misses: u32,
    log: ConnLog,
}

impl DevWorker {
    fn run(&mut self, rx: Receiver<DevCmd>) {
        winaudio::com_init();
        let mut next_poll = Instant::now();
        loop {
            let wait = next_poll.saturating_duration_since(Instant::now()).min(EVENT_TICK);
            match rx.recv_timeout(wait) {
                Ok(first) => {
                    let mut batch = vec![first];
                    batch.extend(rx.try_iter());
                    self.handle(batch);
                    // Re-read state soon after a write.
                    next_poll = next_poll.min(Instant::now() + Duration::from_millis(1500));
                }
                Err(RecvTimeoutError::Timeout) if Instant::now() < next_poll => {
                    if self.check_events() {
                        next_poll = Instant::now();
                    }
                }
                Err(RecvTimeoutError::Timeout) if self.diag => {
                    self.check_events();
                    next_poll = Instant::now() + POLL_CONNECTED;
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.poll();
                    let every = if self.info.status.headset_connected { POLL_CONNECTED } else { POLL_DISCONNECTED };
                    next_poll = Instant::now() + every;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    /// Log a link change, unless the background watcher is running (it
    /// keeps the log then, and two writers would duplicate every line).
    fn log_link(&mut self, connected: bool, how: &str) {
        if !self.demo && !crate::instance::watcher_running() {
            self.log.update(connected, how);
        }
    }

    /// Handle link events the headset sent on its own. These catch drops
    /// too short for the periodic poll. Returns true if a poll is due now
    /// (the headset came back and needs its profile).
    fn check_events(&mut self) -> bool {
        let Some(dev) = &self.dev else { return false };
        let events = match dev.take_events(0) {
            Ok(e) => e,
            Err(_) => {
                self.dev = None;
                return true;
            }
        };
        let mut poll_now = false;
        for e in &events {
            dlog!("aviso del headset: {:02X} {:?}", e.cmd, e.data);
            // The EQ button reports the new preset. A select of ours that
            // briefly lands elsewhere does too, so this only triggers a read:
            // what the headset plays once our write is done decides.
            if e.preset().is_some() && !self.diag {
                self.button_preset = true;
                poll_now = true;
            }
        }
        for link in events.iter().filter_map(|e| e.link()) {
            self.log_link(link, "aviso del headset");
            if link {
                poll_now = true;
            } else if self.info.status.headset_connected {
                // Trust the headset's own report: no debounce.
                self.misses = 2;
                let mut info = self.info.clone();
                info.status = Status::default();
                self.set_info(info);
            }
        }
        poll_now
    }

    fn emit(&self, ev: DevEvent) {
        let _ = self.tx.send(ev);
        (self.notify)();
    }

    fn message(&self, text: impl Into<String>, error: bool) {
        self.emit(DevEvent::Message { text: text.into(), error });
    }

    fn set_info(&mut self, info: DeviceInfo) {
        if info != self.info {
            self.info = info;
            self.emit(DevEvent::Info(self.info.clone()));
        }
    }

    /// Process a batch of commands.
    fn handle(&mut self, batch: Vec<DevCmd>) {
        let mut changes: Vec<Change> = Vec::new();
        for cmd in batch {
            match cmd {
                DevCmd::Update(t, change) => {
                    self.target = t;
                    if !changes.contains(&change) {
                        changes.push(change);
                    }
                }
                DevCmd::SetTarget(t) => self.target = t,
                DevCmd::Diag(d) => {
                    self.diag_cmd(d);
                    changes.clear();
                }
            }
        }
        if let Some(dev) = &self.dev {
            dev.set_options(self.target.opts);
        }
        if changes.contains(&Change::All) {
            changes = vec![Change::All];
        }
        changes.sort();
        if !changes.is_empty() && !self.diag {
            self.push(&changes);
        }
    }

    fn set_busy(&mut self, busy: bool) {
        let mut info = self.info.clone();
        info.busy = busy;
        self.set_info(info);
    }

    /// One step of the guided test.
    fn diag_cmd(&mut self, cmd: DiagCmd) {
        match cmd {
            DiagCmd::Begin => {
                dlog!("=== prueba guiada: inicio ===");
                self.diag = true;
                if self.dev.is_none() && !self.demo {
                    self.dev = Device::open(0).ok();
                }
                if let Some(dev) = &self.dev {
                    dev.set_options(self.target.opts);
                    dev.hold_bus(true);
                }
            }
            DiagCmd::Run { action, method, release } => {
                dlog!("=== prueba guiada: {action:?}, método {method:?}, liberar remoto {release} ===");
                self.set_busy(true);
                let text = if self.demo {
                    thread::sleep(Duration::from_millis(400));
                    format!("(demo) {action:?}")
                } else {
                    match &self.dev {
                        None => "Dongle no encontrado".to_string(),
                        Some(dev) => {
                            dev.set_options(Options { method, release_remote: release, ..self.target.opts });
                            let result = match action {
                                DiagAction::CurveRelatch(bands) => dev.set_curve_relatched(EqPreset::Custom, &bands),
                                DiagAction::SlotCurve(p, bands) => dev.set_slot_curve(p, &bands),
                                DiagAction::Select(p) => dev.set_preset(p),
                            };
                            thread::sleep(Duration::from_millis(150));
                            let back = dev.eq_readback();
                            match result {
                                Ok(()) => format!("Enviado. El headset dice: {back}"),
                                Err(e) => format!("Error: {e}. El headset dice: {back}"),
                            }
                        }
                    }
                };
                dlog!("resultado: {text}");
                self.last_write = Instant::now();
                self.set_busy(false);
                self.emit(DevEvent::Diag(text));
            }
            DiagCmd::End(target) => {
                dlog!("=== prueba guiada: fin, opciones {:?} ===", target.opts);
                self.diag = false;
                self.target = target;
                if let Some(dev) = &self.dev {
                    dev.hold_bus(false);
                    dev.set_options(self.target.opts);
                }
                self.push(&[Change::All]);
            }
        }
    }

    /// The panel opened with the headset already on: its EQ button may have
    /// switched presets while no rzr was running, so keep the headset's
    /// preset rather than overwrite it with the profile's. Not on a later
    /// connect: a headset that was off may not be where it was left.
    fn adopt_headset_preset(&mut self) {
        let Some(p) = self.info.status.preset else { return };
        if p != self.target.profile.active_preset() {
            dlog!("al abrir, el headset está en {p:?}, el perfil dice {:?}", self.target.profile.active_preset());
            self.target.profile.select_preset(p);
            self.emit(DevEvent::PresetChanged { preset: p, from_button: true });
        }
    }

    /// Send changes to the headset, if it's there to receive them.
    fn push(&mut self, changes: &[Change]) {
        dlog!("enviar {changes:?}");
        if !self.info.status.headset_connected {
            self.message("Guardado. Se aplicará cuando el headset se conecte.", false);
            return;
        }
        self.set_busy(true);
        let result = self.write(changes);
        self.last_write = Instant::now();
        if crate::debuglog::enabled() && result.is_ok() && !self.demo {
            if let Some(dev) = &self.dev {
                dev.eq_readback();
            }
        }
        self.set_busy(false);

        if let Err(e) = result {
            dlog!("error al enviar: {e}");
            self.message(format!("No se pudo aplicar: {e}"), true);
            if e != device::BUSY {
                self.dev = None; // reopen on next poll
            }
        } else if changes == [Change::All] {
            self.message(format!("Perfil «{}» aplicado", self.target.profile.name), false);
        }
    }

    fn write(&mut self, changes: &[Change]) -> Result<(), String> {
        if self.demo {
            thread::sleep(Duration::from_millis(300));
            return Ok(());
        }
        let dev = self.dev.as_ref().ok_or("Dongle no encontrado")?;
        let p = &self.target.profile;
        for change in changes {
            match change {
                Change::All => dev.apply_profile(p)?,
                Change::Eq => dev.set_eq(p.active_preset(), &p.custom_eq)?,
                Change::Sidetone => dev.set_sidetone(p.sidetone_wire())?,
                Change::Dnd => dev.set_dnd(p.dnd)?,
                Change::AutoOff => dev.set_auto_off(p.auto_off_wire())?,
                Change::MicEq => dev.set_mic_eq_preset(p.mic.eq_preset.selector())?,
            }
        }
        Ok(())
    }

    fn read_status(&mut self) -> Option<Status> {
        if self.demo {
            return Some(Status {
                headset_connected: true,
                battery: Some(70),
                charging: Some(false),
                mic_muted: Some(false),
                preset: Some(self.target.profile.active_preset()),
            });
        }
        if self.dev.is_none() {
            self.dev = Device::open(0).ok();
            if let Some(dev) = &self.dev {
                dev.set_options(self.target.opts);
            }
        }
        let dev = self.dev.as_ref()?;
        match dev.status() {
            Ok(st) => Some(st),
            // The watcher is mid-sequence: keep the last state.
            Err(e) if e == device::BUSY => Some(self.info.status.clone()),
            Err(e) => {
                dlog!("error al leer el estado: {e}");
                self.dev = None;
                None
            }
        }
    }

    fn poll(&mut self) {
        let was_connected = self.info.status.headset_connected;
        let first = std::mem::take(&mut self.first_poll);
        let Some(status) = self.read_status() else {
            self.log_link(false, "dongle no encontrado");
            self.set_info(DeviceInfo::default());
            return;
        };

        if status.headset_connected {
            self.misses = 0;
        } else {
            self.misses += 1;
            if was_connected && self.misses < 2 {
                return;
            }
        }

        let mut info = self.info.clone();
        info.dongle = true;
        info.product = match &self.dev {
            Some(d) => d.product.clone(),
            None => "Razer BlackShark V2 Pro (demo)".to_string(),
        };
        info.status = status;
        let connected = info.status.headset_connected;
        self.log_link(connected, "comprobación periódica");

        // The dongle answers this even with the headset off.
        if info.dongle_firmware.is_none() {
            info.dongle_firmware = match &self.dev {
                Some(dev) => dev.get_dongle_firmware(),
                None => Some("2.4.1.0".into()), // demo
            };
        }
        if connected && info.firmware.is_none() {
            if self.demo {
                info.firmware = Some("v1.3".into());
                info.serial = Some("DEMO000000000".into());
            } else if let Some(dev) = &self.dev {
                info.firmware = dev.get_firmware();
                info.serial = dev.get_serial();
            }
        }
        if !connected {
            info.firmware = None;
            info.serial = None;
        }
        self.set_info(info);

        if connected && !was_connected {
            // Headset just connected (or first detection): push everything,
            // unless the background watcher is already doing it — two
            // interleaved apply sequences would garble each other.
            if !crate::instance::watcher_running() {
                if first {
                    self.adopt_headset_preset();
                }
                self.push(&[Change::All]);
            }
            return;
        }

        // Preset switched from the headset's EQ button, or our write
        // didn't take? Either way, show what the headset really plays.
        let from_button = std::mem::take(&mut self.button_preset);
        if let Some(p) = self.info.status.preset {
            let wanted = self.target.profile.active_preset();
            if connected && preset_drifted(p, wanted, from_button, self.last_write.elapsed()) {
                dlog!("el headset está en {p:?}, el perfil dice {wanted:?} (botón: {from_button})");
                self.target.profile.select_preset(p);
                self.emit(DevEvent::PresetChanged { preset: p, from_button });
            }
        }
    }
}

/// Whether the headset's preset `read` should replace the profile's `wanted`
/// one. After a preset event it should right away: the read comes after our
/// own write finished, so it is what the headset really plays. Without one,
/// only once our last write has settled.
fn preset_drifted(read: EqPreset, wanted: EqPreset, preset_event: bool, since_write: Duration) -> bool {
    read != wanted && (preset_event || since_write > PRESET_SETTLE)
}

// ---------------------------------------------------------------------------
// Windows audio

pub enum AudioCmd {
    Volume(String, f32),
    Mute(String, bool),
    /// Make this endpoint the Windows default, once: the user picked it in
    /// the panel. rzr never changes the default on its own (on connect or at
    /// startup), since the user switches devices themselves.
    DefaultDevice(String),
}

/// Switch the Windows default and log it by name: the change is visible to
/// the user, and hard to trace otherwise.
fn set_default_device(id: &str) {
    let set = winaudio::set_default_device(id);
    let name = [Flow::Render, Flow::Capture]
        .into_iter()
        .flat_map(winaudio::list_devices)
        .find(|d| d.id == id)
        .map_or_else(|| id.to_string(), |d| d.name);
    dlog!("predeterminado elegido en el panel: {name} ({})", if set { "hecho" } else { "falló" });
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioState {
    pub outputs: Vec<AudioDevice>,
    pub inputs: Vec<AudioDevice>,
    pub headset_out: Option<Endpoint>,
    pub headset_in: Option<Endpoint>,
}

pub fn spawn_audio_worker(demo: bool, notify: Notify) -> (Sender<AudioCmd>, Receiver<AudioState>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<AudioCmd>();
    let (st_tx, st_rx) = mpsc::channel();
    thread::spawn(move || {
        winaudio::com_init();
        let mut demo_state = demo.then(demo_audio);
        let mut last = None;
        loop {
            match cmd_rx.recv_timeout(Duration::from_secs(2)) {
                Ok(first) => {
                    let mut batch = vec![first];
                    batch.extend(cmd_rx.try_iter());
                    // Only the last volume per endpoint matters.
                    let mut volumes: Vec<(String, f32)> = Vec::new();
                    for cmd in batch {
                        match cmd {
                            AudioCmd::Volume(id, v) => {
                                volumes.retain(|(i, _)| *i != id);
                                volumes.push((id, v));
                            }
                            AudioCmd::Mute(id, m) => match &mut demo_state {
                                Some(s) => demo_update(s, &id, |e| e.muted = m),
                                None => {
                                    winaudio::set_mute(&id, m);
                                }
                            },
                            AudioCmd::DefaultDevice(id) => match &mut demo_state {
                                Some(s) => demo_set_default(s, &id),
                                None => set_default_device(&id),
                            },
                        }
                    }
                    for (id, v) in volumes {
                        match &mut demo_state {
                            Some(s) => demo_update(s, &id, |e| e.volume = v),
                            None => {
                                winaudio::set_volume(&id, v);
                            }
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let state = match &demo_state {
                Some(s) => s.clone(),
                None => AudioState {
                    outputs: winaudio::list_devices(Flow::Render),
                    inputs: winaudio::list_devices(Flow::Capture),
                    headset_out: winaudio::headset_endpoint(Flow::Render),
                    headset_in: winaudio::headset_endpoint(Flow::Capture),
                },
            };
            if last.as_ref() != Some(&state) {
                last = Some(state.clone());
                if st_tx.send(state).is_err() {
                    break;
                }
                notify();
            }
        }
    });
    (cmd_tx, st_rx)
}

// ---------------------------------------------------------------------------
// THX

/// How often to re-read the THX state (it also changes from Synapse or the
/// headset's EQ button while Synapse is open).
const THX_POLL: Duration = Duration::from_secs(2);
/// How long the service may take to rewrite its JSON after a change (it took
/// 1-3 s on the user's PC).
const THX_CONFIRM: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThxCmd {
    Change(ThxChange),
    /// The THX preset and curve for the profile's headset preset (ADR 0006).
    Eq(ThxEq),
    /// The profile's microphone enhancements (ADR 0007).
    Mic(MicSettings),
}

impl ThxCmd {
    /// Output settings live in the THX state; the microphone's don't.
    fn apply(self, s: &mut ThxState) {
        match self {
            ThxCmd::Change(c) => c.apply(s),
            ThxCmd::Eq(eq) => eq.apply(s),
            ThxCmd::Mic(_) => {}
        }
    }

    fn shown_in(self, s: &ThxState) -> bool {
        match self {
            ThxCmd::Change(c) => c.shown_in(s),
            ThxCmd::Eq(eq) => eq.shown_in(s),
            ThxCmd::Mic(_) => false,
        }
    }

    fn label(self) -> String {
        match self {
            ThxCmd::Change(c) => c.label(),
            ThxCmd::Eq(_) => "Ecualizador de THX".into(),
            ThxCmd::Mic(_) => "Mejoras del micrófono".into(),
        }
    }

    /// Commands where only the latest one matters.
    fn replaces(self, other: ThxCmd) -> bool {
        matches!((self, other), (ThxCmd::Eq(_), ThxCmd::Eq(_)) | (ThxCmd::Mic(_), ThxCmd::Mic(_)))
    }
}

/// The commands to run from a batch that arrived together: only the last EQ
/// and the last microphone settings count (dragging a curve or a slider sends
/// one per release), the rest in order.
fn thx_batch(cmds: Vec<ThxCmd>) -> Vec<ThxCmd> {
    let later = |i: usize, c: ThxCmd| cmds[i + 1..].iter().any(|&n| n.replaces(c));
    cmds.iter().enumerate().filter(|&(i, &c)| !later(i, c)).map(|(_, &c)| c).collect()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThxStatus {
    /// None: THX isn't installed on the headset's output (or it isn't there).
    pub state: Option<ThxState>,
    /// The THX service answers, so settings can be changed.
    pub service: bool,
    pub busy: bool,
}

pub enum ThxEvent {
    Status(ThxStatus),
    Message { text: String, error: bool },
}

pub fn spawn_thx_worker(demo: bool, notify: Notify) -> (Sender<ThxCmd>, Receiver<ThxEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (ev_tx, ev_rx) = mpsc::channel();
    thread::spawn(move || {
        winaudio::com_init();
        let mut w = ThxWorker {
            demo,
            tx: ev_tx,
            notify,
            status: ThxStatus::default(),
            service: None,
            eq: None,
            mic: None,
            last_problem: String::new(),
        };
        if demo {
            w.status = ThxStatus { state: Some(demo_thx()), service: true, busy: false };
            w.emit(ThxEvent::Status(w.status.clone()));
        }
        loop {
            match cmd_rx.recv_timeout(THX_POLL) {
                Ok(first) => {
                    let batch = std::iter::once(first).chain(cmd_rx.try_iter()).collect();
                    for cmd in thx_batch(batch) {
                        w.run(cmd);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if !demo {
                w.poll();
            }
        }
    });
    (cmd_tx, ev_rx)
}

struct ThxWorker {
    demo: bool,
    tx: Sender<ThxEvent>,
    notify: Notify,
    status: ThxStatus,
    service: Option<thx::Service>,
    /// The last THX EQ asked for, put back after a Spatial switch: THX keeps
    /// another curve per preset with Spatial on.
    eq: Option<ThxEq>,
    /// The profile's microphone settings. THX forgets them when its service
    /// restarts, so they're sent again whenever rzr (re)connects to it.
    mic: Option<MicSettings>,
    /// Last problem logged, so a lasting one is logged once and not every poll.
    last_problem: String,
}

impl ThxWorker {
    fn emit(&self, ev: ThxEvent) {
        let _ = self.tx.send(ev);
        (self.notify)();
    }

    fn message(&self, text: String, error: bool) {
        self.emit(ThxEvent::Message { text, error });
    }

    fn update(&mut self, status: ThxStatus) {
        if status != self.status {
            self.status = status;
            self.emit(ThxEvent::Status(self.status.clone()));
        }
    }

    fn problem(&mut self, text: String) {
        if text != self.last_problem {
            dlog!("THX: {text}");
            self.last_problem = text;
        }
    }

    /// The THX state of the headset's output, if THX is on it.
    fn read(&mut self) -> Option<ThxState> {
        let endpoint = winaudio::headset_endpoint(Flow::Render)?;
        match thx::read_state(&endpoint.id)? {
            Ok(s) => Some(s),
            Err(e) => {
                self.problem(e);
                None
            }
        }
    }

    fn poll(&mut self) {
        let state = self.read();
        if self.service.as_ref().is_some_and(|s| !s.alive()) {
            self.problem("se perdió la conexión con el servicio de THX".into());
            self.service = None;
        }
        if state.is_some() && self.service.is_none() {
            match thx::Service::connect() {
                Ok(s) => {
                    self.service = Some(s);
                    self.apply_mic(false);
                }
                Err(e) => self.problem(e),
            }
        }
        let service = state.is_some() && self.service.is_some();
        self.update(ThxStatus { state, service, busy: self.status.busy });
    }

    /// Send the microphone settings. `asked`: the user just changed them, so
    /// a failure is shown; otherwise it's only logged.
    fn apply_mic(&mut self, asked: bool) {
        let (Some(service), Some(mic)) = (&self.service, self.mic) else { return };
        if self.demo {
            return;
        }
        dlog!("THX: micrófono {mic:?}");
        match service.set_mic(&mic) {
            Ok(()) => dlog!("THX: micrófono confirmado"),
            Err(e) if asked => {
                dlog!("THX: {e}");
                self.message(format!("Mejoras del micrófono: {e}"), true);
            }
            Err(e) => self.problem(e),
        }
    }

    /// Make a change, then wait until the service's stored state shows it.
    fn run(&mut self, cmd: ThxCmd) {
        match cmd {
            ThxCmd::Eq(eq) => self.eq = Some(eq),
            ThxCmd::Mic(mic) => {
                self.mic = Some(mic);
                self.update(ThxStatus { busy: true, ..self.status.clone() });
                self.apply_mic(true);
                self.update(ThxStatus { busy: false, ..self.status.clone() });
                return;
            }
            ThxCmd::Change(_) => {}
        }
        let Some(before) = self.status.state.clone() else { return };
        if cmd.shown_in(&before) {
            return;
        }
        dlog!("THX: {cmd:?}");
        if self.demo {
            let mut status = self.status.clone();
            if let Some(s) = &mut status.state {
                cmd.apply(s);
            }
            self.update(status);
            return;
        }
        // Show the requested value while waiting; a failure puts back the real one.
        let mut pending = before.clone();
        cmd.apply(&mut pending);
        self.update(ThxStatus { state: Some(pending), busy: true, ..self.status.clone() });

        let result = match (&self.service, cmd) {
            (Some(service), ThxCmd::Change(change)) => service.apply(change),
            (Some(service), ThxCmd::Eq(eq)) => service.set_eq(&eq),
            (Some(_), ThxCmd::Mic(_)) => Ok(()),
            (None, _) => Err("el servicio de THX no responde".to_string()),
        };
        let error = match result {
            Ok(()) => self.confirm(cmd, before.sequence_number),
            Err(e) => {
                // Reconnect next time: the service may have restarted.
                self.service = None;
                Some(e)
            }
        };
        let failed = error.is_some();
        if let Some(e) = error {
            dlog!("THX: {e}");
            self.message(format!("{}: {e}", cmd.label()), true);
        }
        self.update(ThxStatus { busy: false, ..self.status.clone() });
        self.poll();
        // Synapse doesn't do this, and its EQ seems to vanish when Spatial is switched.
        if let (ThxCmd::Change(ThxChange::Switch(ThxOption::Spatial, _)), Some(eq), false) = (cmd, self.eq, failed) {
            self.run(ThxCmd::Eq(eq));
        }
    }

    /// Wait for the stored state to show the change. None when it does.
    fn confirm(&mut self, change: ThxCmd, seq_before: u64) -> Option<String> {
        let deadline = Instant::now() + THX_CONFIRM;
        while Instant::now() < deadline {
            thread::sleep(Duration::from_millis(250));
            if let Some(s) = self.read() {
                if s.sequence_number > seq_before && change.shown_in(&s) {
                    dlog!("THX: confirmado, secuencia {}", s.sequence_number);
                    self.update(ThxStatus { state: Some(s), ..self.status.clone() });
                    return None;
                }
            }
        }
        Some("el servicio aceptó el cambio pero su estado guardado no lo muestra".to_string())
    }
}

fn demo_thx() -> ThxState {
    ThxState {
        sequence_number: 1,
        preset_name: "Music Mode".into(),
        spatial_enabled: false,
        bass_boost_enabled: true,
        bass_boost: 50.0,
        drc_enabled: false,
        drc_level: 100.0,
        dialog_enhancement_enabled: false,
        dialog_enhancement: 100.0,
        eq_curve: ThxEq::new(EqPreset::Music, EqPreset::Music.curve().unwrap_or_default()).gains().to_vec(),
    }
}

fn demo_audio() -> AudioState {
    let out = AudioDevice {
        name: "Auriculares (Razer BlackShark V2 Pro 2.4)".into(),
        id: "demo-out".into(),
        is_default: true,
    };
    let inp =
        AudioDevice { name: "Micrófono (Razer BlackShark V2 Pro 2.4)".into(), id: "demo-in".into(), is_default: true };
    AudioState {
        outputs: vec![
            out.clone(),
            AudioDevice { name: "Altavoces (Realtek(R) Audio)".into(), id: "demo-spk".into(), is_default: false },
        ],
        inputs: vec![inp.clone()],
        headset_out: Some(Endpoint { id: out.id, name: out.name, volume: 0.8, muted: false }),
        headset_in: Some(Endpoint { id: inp.id, name: inp.name, volume: 1.0, muted: false }),
    }
}

/// Make `id` the default of its own list, as Windows does per flow.
fn demo_set_default(s: &mut AudioState, id: &str) {
    for list in [&mut s.outputs, &mut s.inputs] {
        if list.iter().any(|d| d.id == id) {
            for d in list.iter_mut() {
                d.is_default = d.id == id;
            }
        }
    }
}

fn demo_update(s: &mut AudioState, id: &str, f: impl Fn(&mut Endpoint)) {
    for e in [&mut s.headset_out, &mut s.headset_in].into_iter().flatten() {
        if e.id == id {
            f(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thx::ThxLevel;

    #[test]
    fn a_thx_batch_keeps_only_the_last_eq() {
        let eq = |db| ThxCmd::Eq(ThxEq::new(EqPreset::Custom, [db; EQ_BANDS]));
        let spatial = ThxCmd::Change(ThxChange::Switch(ThxOption::Spatial, true));
        let bass = ThxCmd::Change(ThxChange::Level(ThxLevel::BassBoost, 40.0));
        assert_eq!(thx_batch(vec![eq(1), spatial, eq(2), bass, eq(3)]), vec![spatial, bass, eq(3)]);
        assert_eq!(thx_batch(vec![spatial, bass]), vec![spatial, bass]);
        assert_eq!(thx_batch(vec![eq(1)]), vec![eq(1)]);
        let mic = |on| {
            let mut m = MicSettings::default();
            m.normalization.on = on;
            ThxCmd::Mic(m)
        };
        assert_eq!(thx_batch(vec![mic(true), eq(1), mic(false), eq(2)]), vec![mic(false), eq(2)]);
    }

    #[test]
    fn the_eq_button_counts_right_after_our_own_writes() {
        let just_now = Duration::from_millis(500);
        let settled = PRESET_SETTLE + Duration::from_millis(1);
        // Button pressed 0.5 s after a write from the panel.
        assert!(preset_drifted(EqPreset::Music, EqPreset::Movie, true, just_now));
        // A select of ours that briefly landed elsewhere: the read shows ours.
        assert!(!preset_drifted(EqPreset::Fortnite, EqPreset::Fortnite, true, just_now));
        // No event: a mismatch right after a write may still be settling.
        assert!(!preset_drifted(EqPreset::Valorant, EqPreset::Fortnite, false, just_now));
        assert!(preset_drifted(EqPreset::Valorant, EqPreset::Fortnite, false, settled));
    }
}
