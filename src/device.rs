/// HID device communication for the Razer BlackShark V2 Pro dongle.
///
/// Write sequences mirror what Synapse sends and what the OpenRazer driver
/// verified on this headset. Two firmware quirks shape them:
///
/// - The 2.4GHz link dozes after ~0.3s idle and drops the first frame sent
///   into it, so every sequence starts with a sacrificial remote-mode frame.
/// - A preset select that crosses families (classic <-> esports) only
///   switches the family, so selects are read back and retried.

use hidapi::HidApi;
use std::cell::RefCell;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Profile;
use crate::protocol::{self as proto, EqPreset, Packet, EQ_BANDS};

const VID: u16 = 0x1532;
const PID: u16 = 0x0555;
const USAGE_PAGE_VENDOR: u16 = 0xFF00;
const SLEEP_BETWEEN_CMDS: Duration = Duration::from_millis(35);
/// Per-attempt reply wait; must stay below the link's doze threshold.
const REPLY_TIMEOUT: Duration = Duration::from_millis(150);
const QUERY_ATTEMPTS: usize = 3;

pub struct Device {
    handle: hidapi::HidDevice,
    pub product: String,
    /// Unsolicited event frames seen while reading replies.
    events: RefCell<Vec<Event>>,
}

/// An unsolicited frame from the headset (link, battery, mic mute, DND...).
#[derive(Clone, Debug)]
pub struct Event {
    pub cmd: u8,
    pub data: Vec<u8>,
}

impl Event {
    /// Link state carried by a "WIRELESS_CONNECTION_STATUS" event.
    pub fn link(&self) -> Option<bool> {
        (self.cmd == proto::CMD_WIRELESS).then(|| self.data.first() == Some(&1))
    }
}

/// Snapshot of the headset's read-only state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub headset_connected: bool,
    pub battery: Option<u8>,
    pub charging: Option<bool>,
    pub mic_muted: Option<bool>,
    pub preset: Option<EqPreset>,
}

impl Device {
    /// Find and open the vendor-defined HID endpoint.
    /// Retries until timeout_ms expires.
    pub fn open(timeout_ms: u32) -> Result<Self, String> {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);

        loop {
            let api = HidApi::new().map_err(|e| format!("HID init failed: {e}"))?;

            for info in api.device_list() {
                if info.vendor_id() == VID
                    && info.product_id() == PID
                    && info.usage_page() == USAGE_PAGE_VENDOR
                {
                    let handle = info
                        .open_device(&api)
                        .map_err(|e| format!("Cannot open device: {e}"))?;
                    let product = info
                        .product_string()
                        .unwrap_or("Razer BlackShark V2 Pro")
                        .to_string();
                    return Ok(Device {
                        handle,
                        product,
                        events: RefCell::new(Vec::new()),
                    });
                }
            }

            if Instant::now() >= deadline {
                return Err("Device not found. Is the dongle plugged in?".to_string());
            }

            thread::sleep(Duration::from_millis(500));
        }
    }

    /// Send a 64-byte packet.
    fn send(&self, pkt: &Packet) -> Result<(), String> {
        self.handle
            .write(pkt)
            .map_err(|e| format!("Write failed: {e}"))?;
        thread::sleep(SLEEP_BETWEEN_CMDS);
        Ok(())
    }

    fn remote(&self, on: bool) -> Result<(), String> {
        self.send(&proto::set_remote_mode(on))
    }

    /// Keep the event frames of an input report.
    fn collect_events(&self, report: &[u8]) {
        let mut events = self.events.borrow_mut();
        for f in proto::frames(report) {
            if f.flag == proto::FLAG_EVENT {
                events.push(Event { cmd: f.cmd, data: f.data.to_vec() });
            }
        }
    }

    /// Drop queued replies so a stale one can't answer a new query
    /// (events are kept).
    fn drain(&self) {
        let mut buf = [0u8; 64];
        while let Ok(n) = self.handle.read_timeout(&mut buf, 0) {
            if n == 0 {
                break;
            }
            self.collect_events(&buf[..n]);
        }
    }

    /// Events received so far, waiting up to `wait_ms` for new ones.
    /// Err means the dongle is gone.
    pub fn take_events(&self, wait_ms: i32) -> Result<Vec<Event>, String> {
        let mut buf = [0u8; 64];
        let mut timeout = wait_ms;
        loop {
            match self.handle.read_timeout(&mut buf, timeout) {
                Ok(0) => break,
                Ok(n) => self.collect_events(&buf[..n]),
                Err(e) => return Err(format!("Read failed: {e}")),
            }
            timeout = 0; // after the first report, only take what's queued
        }
        Ok(std::mem::take(&mut *self.events.borrow_mut()))
    }

    /// Read input reports until `pick` accepts one, up to REPLY_TIMEOUT.
    fn read_matching<T>(&self, pick: impl Fn(&[u8]) -> Option<T>) -> Option<T> {
        let deadline = Instant::now() + REPLY_TIMEOUT;
        let mut buf = [0u8; 64];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            match self.handle.read_timeout(&mut buf, left.as_millis() as i32) {
                Ok(n) if n > 0 => {
                    self.collect_events(&buf[..n]);
                    if let Some(v) = pick(&buf[..n]) {
                        return Some(v);
                    }
                }
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }

    /// Wait for the reply to `cmd_id`, up to REPLY_TIMEOUT.
    fn read_reply(&self, cmd_id: u8) -> Option<Vec<u8>> {
        self.read_matching(|buf| proto::parse_reply(buf, cmd_id).map(|p| p.to_vec()))
    }

    /// Read a register. Wake, then (remote on, query) up to QUERY_ATTEMPTS
    /// times, then hand control back to the headset.
    /// Ok(None) = no reply (headset off or out of range); Err = I/O failure
    /// (dongle unplugged).
    pub fn query(&self, cmd_id: u8) -> Result<Option<Vec<u8>>, String> {
        self.drain();
        self.remote(true)?; // wake frame, may be dropped
        let mut result = None;
        for _ in 0..QUERY_ATTEMPTS {
            self.remote(true)?;
            self.handle
                .write(&proto::query(cmd_id))
                .map_err(|e| format!("Write failed: {e}"))?;
            if let Some(payload) = self.read_reply(cmd_id) {
                result = Some(payload);
                break;
            }
        }
        self.remote(false)?;
        Ok(result)
    }

    fn query_u8(&self, cmd_id: u8) -> Result<Option<u8>, String> {
        Ok(self.query(cmd_id)?.and_then(|v| v.first().copied()))
    }

    /// Whether the headset is linked. Err means the dongle is gone.
    pub fn link_status(&self) -> Result<bool, String> {
        Ok(self.query_u8(proto::CMD_WIRELESS)? == Some(1))
    }

    /// Check if headset is wirelessly connected to the dongle.
    pub fn is_headset_connected(&self) -> bool {
        matches!(self.query_u8(proto::CMD_WIRELESS), Ok(Some(1)))
    }

    /// Get battery percentage. Returns None if unavailable.
    pub fn get_battery(&self) -> Option<u8> {
        self.query_u8(proto::CMD_BATTERY).ok().flatten().map(|b| b.min(100))
    }

    pub fn is_charging(&self) -> Option<bool> {
        self.query_u8(proto::CMD_CHARGING).ok().flatten().map(|s| s != 0)
    }

    /// State of the headset's hardware mic-mute button.
    pub fn is_mic_muted(&self) -> Option<bool> {
        self.query_u8(proto::CMD_MIC_MUTE).ok().flatten().map(|s| s != 0)
    }

    pub fn get_preset(&self) -> Option<EqPreset> {
        self.query_u8(proto::CMD_PRESET_GET)
            .ok()
            .flatten()
            .and_then(EqPreset::from_selector)
    }

    pub fn get_firmware(&self) -> Option<String> {
        let v = self.query(proto::CMD_FIRMWARE).ok()??;
        (v.len() >= 2).then(|| format!("v{}.{}", v[0], v[1]))
    }

    /// Firmware of the USB dongle itself (answers even with the headset off).
    pub fn get_dongle_firmware(&self) -> Option<String> {
        self.drain();
        self.remote(true).ok()?;
        self.handle.write(&proto::get_dongle_firmware()).ok()?;
        let fw = self.read_matching(proto::parse_dongle_firmware);
        self.remote(false).ok()?;
        fw
    }

    pub fn get_serial(&self) -> Option<String> {
        let v = self.query(proto::CMD_SERIAL).ok()??;
        let s: String = v
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect();
        let s = s.trim().to_string();
        (!s.is_empty()).then_some(s)
    }

    /// Read the headset's state in one go. Skips the rest if the headset
    /// isn't linked (the dongle answers nothing for it then).
    /// Err means the dongle itself is gone.
    pub fn status(&self) -> Result<Status, String> {
        let mut st = Status {
            headset_connected: self.query_u8(proto::CMD_WIRELESS)? == Some(1),
            ..Status::default()
        };
        if st.headset_connected {
            st.battery = self.get_battery();
            st.charging = self.is_charging();
            st.mic_muted = self.is_mic_muted();
            st.preset = self.get_preset();
        }
        Ok(st)
    }

    /// Selector-only preset apply, as captured from Synapse:
    /// remote x2 -> prep query -> preset -> family flag -> remote.
    fn apply_preset_once(&self, preset: EqPreset) -> Result<(), String> {
        self.remote(true)?;
        self.remote(true)?;
        self.send(&proto::query(proto::CMD_PREP))?;
        self.remote(true)?;
        self.send(&proto::set_preset(preset))?;
        self.remote(true)?;
        self.send(&proto::set_mode_flag(preset))?;
        self.remote(true)
    }

    /// Select a preset and confirm it by reading it back.
    pub fn set_preset(&self, preset: EqPreset) -> Result<(), String> {
        for _ in 0..4 {
            self.apply_preset_once(preset)?;
            thread::sleep(Duration::from_millis(100));
            match self.query_u8(proto::CMD_PRESET_GET)? {
                Some(sel) if sel == preset.selector() => return Ok(()),
                Some(_) => continue, // landed on the other family's preset: retry
                None => {
                    // No read-back available: send once more, like Synapse.
                    return self.apply_preset_once(preset);
                }
            }
        }
        Err(format!("El headset no confirmó el preset {}", preset.label()))
    }

    /// Write a curve into a preset's slot (Custom or esports) and make it
    /// audible.
    ///
    /// The headset stores a 0x95 frame into whichever preset is active, so
    /// the target preset must be confirmed first. Within one sequence the
    /// selector latches the slot's previous content, so a selector-only
    /// re-apply after the write is what makes the new curve audible.
    fn set_slot_curve(&self, preset: EqPreset, bands: &[i8; EQ_BANDS]) -> Result<(), String> {
        if self.get_preset() != Some(preset) {
            self.set_preset(preset)?;
        }

        self.remote(true)?;
        self.remote(true)?;
        self.send(&proto::query(proto::CMD_PREP))?;
        self.remote(true)?;
        self.send(&proto::set_preset(preset))?;
        self.remote(true)?;
        self.send(&proto::set_mode_flag(preset))?;
        self.remote(true)?;
        self.send(&proto::set_eq_bands(bands))?;
        self.remote(true)?;

        thread::sleep(Duration::from_millis(100));
        self.apply_preset_once(preset)
    }

    /// Apply the EQ side of a profile. Game/Music/Movie are built into the
    /// headset and only selected; Synapse also writes each esports preset's
    /// curve into its slot, so rzr does the same.
    pub fn set_eq(&self, preset: EqPreset, custom: &[i8; EQ_BANDS]) -> Result<(), String> {
        if preset == EqPreset::Custom {
            self.set_slot_curve(preset, custom)
        } else if let (true, Some(curve)) = (preset.is_esports(), preset.curve()) {
            self.set_slot_curve(preset, &curve)
        } else {
            self.set_preset(preset)
        }
    }

    /// Single-byte setting: wake, remote on, value, remote off.
    fn write_value(&self, cmd_id: u8, value: u8) -> Result<(), String> {
        self.remote(true)?;
        self.remote(true)?;
        self.send(&proto::set_value(cmd_id, value))?;
        self.remote(false)
    }

    /// Mic monitoring. 0 = off, otherwise the wire level (see
    /// protocol::sidetone_level). Levels above what OpenRazer saw the headset
    /// accept are read back, and lowered if the headset didn't keep them.
    pub fn set_sidetone(&self, level: u8) -> Result<(), String> {
        let level = level.min(proto::SIDETONE_MAX);
        self.write_value(proto::CMD_SIDETONE, (level > 0) as u8)?;
        if level > 0 {
            self.write_value(proto::CMD_SIDETONE_LEVEL, level)?;
            if level > proto::SIDETONE_SAFE_MAX {
                match self.query_u8(proto::CMD_SIDETONE_LEVEL_GET)? {
                    Some(got) if got != level => {
                        self.write_value(proto::CMD_SIDETONE_LEVEL, proto::SIDETONE_SAFE_MAX)?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn set_dnd(&self, on: bool) -> Result<(), String> {
        self.write_value(proto::CMD_DND, on as u8)
    }

    /// Auto power-off in minutes (15-60), 0 = never.
    pub fn set_auto_off(&self, minutes: u8) -> Result<(), String> {
        let minutes = if minutes == 0 {
            0
        } else {
            minutes.clamp(proto::AUTO_OFF_MIN_MINUTES, proto::AUTO_OFF_MAX_MINUTES)
        };
        self.write_value(proto::CMD_AUTO_OFF, minutes)
    }

    /// Apply the full profile to the headset.
    pub fn apply_profile(&self, profile: &Profile, synapse_init: bool) -> Result<(), String> {
        if synapse_init {
            // Synapse's startup: query the dongle, then clear "Speaker Preset
            // EQ Status" (0x9E). OpenRazer found no audible effect from 0x9E;
            // it's sent for parity with Synapse (and with earlier rzr releases,
            // which sent the same bytes).
            let _ = self.get_dongle_firmware();
            self.write_value(proto::CMD_PRESET_EQ_STATUS, 0)?;
        }
        self.set_eq(profile.active_preset(), &profile.custom_eq)?;
        self.set_sidetone(profile.sidetone_wire())?;
        self.set_dnd(profile.dnd)?;
        self.set_auto_off(profile.auto_off_wire())?;
        Ok(())
    }
}
