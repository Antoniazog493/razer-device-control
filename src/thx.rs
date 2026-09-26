//! THX Spatial Audio: reading its state and changing its options.
//!
//! THX is an audio effect in Windows, not part of the headset (see
//! docs/RESEARCH.md). Its full state is a JSON string that the THX service
//! (`VSSrv.exe`) keeps in the headset's output endpoint properties; rzr reads
//! it from the registry, because the endpoint's property store cuts strings at
//! 259 characters and the JSON is about 1 KB. Changes go to the service, which
//! then rewrites the JSON, by two roads (ADR 0005):
//! - its COM interface (`IVSSrvTHXSettings`) for what it offers: the THX
//!   Spatial Audio, Bass Boost and Voice Clarity switches (ADR 0004);
//! - ZeroMQ, as Synapse does, for the rest: Sound Normalization and levels.
//!
//! THX also has its own software EQ, with its own presets. Like Synapse, rzr
//! picks the THX preset and curve that go with the headset's (ADR 0006).

#[cfg_attr(not(windows), allow(dead_code))]
mod proto;
#[cfg_attr(not(windows), allow(dead_code))]
mod zmtp;

use serde::Deserialize;

use crate::mic::{MicEffect, MicSettings};
use crate::protocol::{EqPreset, EQ_BANDS};

/// Registry key (under HKLM) with the properties of an output endpoint, from
/// its Core Audio ID (`{0.0.0.00000000}.{guid}`; `{0.0.1…}` are inputs).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn properties_key(endpoint_id: &str) -> Option<String> {
    let guid = endpoint_id.strip_prefix("{0.0.0.")?.split_once("}.")?.1;
    let ok = guid.len() == 38
        && guid.starts_with('{')
        && guid.ends_with('}')
        && guid[1..37].chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    ok.then(|| format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{guid}\Properties"))
}

/// The parts of the THX state rzr shows. The JSON has more (room, emitters,
/// upmix...); unknown fields are ignored.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThxState {
    /// Goes up with every change the service makes.
    pub sequence_number: u64,
    pub preset_name: String,
    pub spatial_enabled: bool,
    pub bass_boost_enabled: bool,
    /// 0-100; kept while Bass Boost is off.
    pub bass_boost: f32,
    /// Sound Normalization.
    pub drc_enabled: bool,
    pub drc_level: f32,
    /// Voice Clarity.
    pub dialog_enhancement_enabled: bool,
    pub dialog_enhancement: f32,
    /// The active THX preset's EQ, in the service's 31-value layout.
    pub eq_curve: Vec<f32>,
}

/// Parse the JSON the THX service stores for the endpoint.
pub fn parse_state(json: &str) -> Result<ThxState, String> {
    serde_json::from_str(json).map_err(|e| format!("unreadable THX state: {e}"))
}

/// A THX option rzr can switch on and off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThxOption {
    Spatial,
    BassBoost,
    Normalization,
    VoiceClarity,
}

impl ThxOption {
    pub fn is_on(self, s: &ThxState) -> bool {
        match self {
            ThxOption::Spatial => s.spatial_enabled,
            ThxOption::BassBoost => s.bass_boost_enabled,
            ThxOption::Normalization => s.drc_enabled,
            ThxOption::VoiceClarity => s.dialog_enhancement_enabled,
        }
    }

    fn set(self, s: &mut ThxState, on: bool) {
        match self {
            ThxOption::Spatial => s.spatial_enabled = on,
            ThxOption::BassBoost => s.bass_boost_enabled = on,
            ThxOption::Normalization => s.drc_enabled = on,
            ThxOption::VoiceClarity => s.dialog_enhancement_enabled = on,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThxOption::Spatial => "THX Spatial Audio",
            ThxOption::BassBoost => "Bass Boost",
            ThxOption::Normalization => "Sound Normalization",
            ThxOption::VoiceClarity => "Voice Clarity",
        }
    }
}

/// A THX option with a level (0-100), kept while the option is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThxLevel {
    BassBoost,
    Normalization,
    VoiceClarity,
}

impl ThxLevel {
    fn get(self, s: &ThxState) -> f32 {
        match self {
            ThxLevel::BassBoost => s.bass_boost,
            ThxLevel::Normalization => s.drc_level,
            ThxLevel::VoiceClarity => s.dialog_enhancement,
        }
    }

    fn set(self, s: &mut ThxState, v: f32) {
        match self {
            ThxLevel::BassBoost => s.bass_boost = v,
            ThxLevel::Normalization => s.drc_level = v,
            ThxLevel::VoiceClarity => s.dialog_enhancement = v,
        }
    }

    fn option(self) -> ThxOption {
        match self {
            ThxLevel::BassBoost => ThxOption::BassBoost,
            ThxLevel::Normalization => ThxOption::Normalization,
            ThxLevel::VoiceClarity => ThxOption::VoiceClarity,
        }
    }
}

/// A change to one THX setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ThxChange {
    Switch(ThxOption, bool),
    Level(ThxLevel, f32),
}

impl ThxChange {
    pub fn apply(self, s: &mut ThxState) {
        match self {
            ThxChange::Switch(o, on) => o.set(s, on),
            ThxChange::Level(l, v) => l.set(s, v),
        }
    }

    /// The state already has this value (levels are whole numbers on the page).
    pub fn shown_in(self, s: &ThxState) -> bool {
        match self {
            ThxChange::Switch(o, on) => o.is_on(s) == on,
            ThxChange::Level(l, v) => (l.get(s) - v).abs() < 0.5,
        }
    }

    pub fn label(self) -> String {
        match self {
            ThxChange::Switch(o, _) => o.label().to_string(),
            ThxChange::Level(l, _) => format!("{} level", l.option().label()),
        }
    }

    /// The `thx.sa.State` field this change sets, for the ZeroMQ road.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn patch(self) -> proto::Patch {
        use proto::{state, Patch};
        match self {
            ThxChange::Switch(ThxOption::Spatial, on) => Patch::Bool(state::SPATIAL_ENABLED, on),
            ThxChange::Switch(ThxOption::BassBoost, on) => Patch::Bool(state::BASS_BOOST_ENABLED, on),
            ThxChange::Switch(ThxOption::Normalization, on) => Patch::Bool(state::DRC_ENABLED, on),
            ThxChange::Switch(ThxOption::VoiceClarity, on) => Patch::Bool(state::DIALOG_ENHANCEMENT_ENABLED, on),
            ThxChange::Level(ThxLevel::BassBoost, v) => Patch::Double(state::BASS_BOOST, v.into()),
            ThxChange::Level(ThxLevel::Normalization, v) => Patch::Double(state::DRC_LEVEL, v.into()),
            ThxChange::Level(ThxLevel::VoiceClarity, v) => Patch::Double(state::DIALOG_ENHANCEMENT, v.into()),
        }
    }
}

/// Talking to the THX service over ZeroMQ, as Synapse does (ADR 0005).
#[cfg_attr(not(windows), allow(dead_code))]
mod zmq {
    use super::{proto, zmtp};
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
    use std::time::Duration;

    /// Where the service listens when its discovery key can't be read.
    pub const DEFAULT_ADDRESS: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 49671));
    const TIMEOUT: Duration = Duration::from_secs(3);

    /// The address in the service's discovery key (`tcp://127.0.0.1:49671`),
    /// if it's a local one.
    pub fn parse_address(url: &str) -> Option<SocketAddr> {
        let addr: SocketAddr = url.trim().strip_prefix("tcp://")?.parse().ok()?;
        addr.ip().is_loopback().then_some(addr)
    }

    /// A registered connection: the service answered `Register` with its state.
    struct Session {
        req: zmtp::Req<TcpStream>,
        /// Synapse sends the address of another socket of its own here; the
        /// service needs the frame but never connects to it (2026-09-26).
        address: Vec<u8>,
        state: Vec<u8>,
    }

    impl Session {
        fn open(addr: SocketAddr) -> Result<Self, String> {
            let io = |e: std::io::Error| format!("could not reach the THX service over ZeroMQ: {e}");
            let stream = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(io)?;
            stream.set_read_timeout(Some(TIMEOUT)).map_err(io)?;
            stream.set_write_timeout(Some(TIMEOUT)).map_err(io)?;
            let address = format!("x-address:tcp://{}", stream.local_addr().map_err(io)?).into_bytes();
            let req = zmtp::Req::handshake(stream)?;
            let mut s = Self { req, address, state: Vec::new() };
            s.state = s.call(&proto::register(std::process::id()))?.state;
            Ok(s)
        }

        /// One request: Synapse's four frames (delimiter, x-address,
        /// x-originator, x-payload). Fails unless the service accepts it.
        fn call(&mut self, payload: &[u8]) -> Result<proto::Reply, String> {
            let payload = [&b"x-payload:"[..], payload].concat();
            let parts = self.req.request(&[&self.address, b"x-originator:rzr", &payload])?;
            for part in &parts {
                if let Some(e) = part.strip_prefix(b"x-Exception:") {
                    return Err(format!("the THX service answered with an error: {}", String::from_utf8_lossy(e)));
                }
            }
            let body = parts
                .iter()
                .find_map(|p| p.strip_prefix(b"x-payload:"))
                .ok_or("the THX service answered without data")?;
            let reply = proto::parse_reply(body)?;
            if reply.status != 0 {
                return Err(format!("the THX service did not accept the change ({}): {}", reply.status, reply.msg));
            }
            Ok(reply)
        }
    }

    /// Send the service its own state with `patch` applied; Ok once its answer
    /// shows the new value.
    pub fn change(addr: SocketAddr, patch: proto::Patch) -> Result<(), String> {
        let mut s = Session::open(addr)?;
        let next = proto::next_state(&s.state, &[patch])?;
        let reply = s.call(&next)?;
        if proto::shows(&reply.state, patch)? {
            Ok(())
        } else {
            Err("the THX service answered, but its state does not show the change".into())
        }
    }

    /// Make `name` the active THX preset, unless it already is.
    pub fn set_preset(addr: SocketAddr, name: &str) -> Result<(), String> {
        let mut s = Session::open(addr)?;
        if proto::string(&s.state, proto::state::PRESET_NAME)? == name {
            return Ok(());
        }
        let reply = s.call(&proto::set_preset(&s.state, name)?)?;
        match proto::string(&reply.state, proto::state::PRESET_NAME)? {
            now if now == name => Ok(()),
            now => Err(format!("the THX service is still on preset \"{now}\" instead of \"{name}\"")),
        }
    }
}

/// The THX preset and EQ curve that go with a headset preset, as Synapse
/// picks them: Game, Movie and Music have THX presets of their own; Custom
/// and the Esports presets all use THX's `Custom` with their curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThxEq {
    pub preset: &'static str,
    pub curve: [i8; EQ_BANDS],
}

impl ThxEq {
    pub fn new(preset: EqPreset, curve: [i8; EQ_BANDS]) -> Self {
        let preset = match preset {
            EqPreset::Game => "Game Mode",
            EqPreset::Movie => "Cinema Mode",
            EqPreset::Music => "Music Mode",
            _ => "Custom",
        };
        Self { preset, curve }
    }

    /// The curve as the service takes it: a 0, then each band three times.
    pub fn gains(&self) -> [f32; 31] {
        let mut g = [0.0; 31];
        for (i, &db) in self.curve.iter().enumerate() {
            g[1 + i * 3..4 + i * 3].fill(f32::from(db));
        }
        g
    }

    pub fn shown_in(&self, s: &ThxState) -> bool {
        s.preset_name == self.preset && s.eq_curve == self.gains()
    }

    pub fn apply(&self, s: &mut ThxState) {
        s.preset_name = self.preset.to_string();
        s.eq_curve = self.gains().to_vec();
    }
}

/// The THX preset's name as the page shows it.
pub fn preset_label(name: &str) -> &str {
    match name {
        "Game Mode" => "Game",
        "Cinema Mode" => "Movie",
        "Music Mode" => "Music",
        "Custom" => "Custom",
        other => other,
    }
}

#[cfg(windows)]
pub use imp::*;

#[cfg(windows)]
mod imp {
    use super::{parse_state, properties_key, zmq, MicEffect, MicSettings, ThxChange, ThxEq, ThxOption, ThxState};
    use crate::protocol::EQ_BANDS;
    use std::net::SocketAddr;
    use windows::core::{interface, s, IUnknown, IUnknown_Vtbl, GUID, HRESULT, PCSTR};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_LOCAL_SERVER};
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    /// `VSSrv.CVSSrvTHXSettings`, registered by the THX service's driver package.
    const CLSID_THX_SETTINGS: GUID = GUID::from_u128(0x2ccfc059_a1c5_4408_bce5_0b23690da7a2);
    /// Size of the `inbandMessage` buffer in the service's type library.
    const INBAND_LEN: usize = 4096;
    /// Who made the change, as the service publishes it to other clients.
    const ORIGINATOR: PCSTR = s!("rzr");
    /// Endpoint property holding the THX state as JSON, as a registry value name.
    const STATE_VALUE: &str = "{d5e8f0ab-4de6-4d91-ab21-68868dda6a4a},6";

    /// The THX service's settings interface. Methods are listed in vtable
    /// order, as in VSSrv's type library (VSSrvLib 1.0), with its names in
    /// snake case (`SetBassBoostState` is `set_bass_boost_state`). rzr calls
    /// only the on/off ones; the rest are here to keep the slots right.
    #[interface("3b3af690-70a6-4dda-bbea-0b96493e9da3")]
    unsafe trait IVSSrvTHXSettings: IUnknown {
        fn init(&self, proc_id: u32) -> HRESULT;
        fn get_spatial_processing_state(&self, enabled: *mut i32) -> HRESULT;
        fn set_spatial_processing_state(&self, o: PCSTR, enabled: i32, sz: *mut u64, msg: *mut u8) -> HRESULT;
        fn get_processing_mode(&self, mode: *mut u32) -> HRESULT;
        fn set_processing_mode(&self, o: PCSTR, mode: u32, sz: *mut u64, msg: *mut u8) -> HRESULT;
        fn get_custom_room_type(&self, room: *mut u32) -> HRESULT;
        fn set_custom_room_type(&self, o: PCSTR, room: u32, sz: *mut u64, msg: *mut u8) -> HRESULT;
        fn get_current_mode_eq_gains(&self, gains: *mut f32) -> HRESULT;
        fn set_current_mode_eq_gains(&self, o: PCSTR, gains: *const f32, sz: *mut u64, msg: *mut u8) -> HRESULT;
        fn reset_current_mode_eq_gains(&self, o: PCSTR, sz: *mut u64, msg: *mut u8) -> HRESULT;
        fn get_listening_mode(&self, headphones: *mut i32) -> HRESULT;
        fn set_listening_mode(&self, o: PCSTR, headphones: i32, sz: *mut u64, msg: *mut u8) -> HRESULT;
        fn get_bass_boost_state(&self, enabled: *mut i32) -> HRESULT;
        fn set_bass_boost_state(&self, o: PCSTR, enabled: i32, sz: *mut u64, msg: *mut u8) -> HRESULT;
        fn get_dialog_enhance_state(&self, enabled: *mut i32) -> HRESULT;
        fn set_dialog_enhance_state(&self, o: PCSTR, enabled: i32, sz: *mut u64, msg: *mut u8) -> HRESULT;
    }

    /// `VSSrv.VSSrvSettings`: the service's other settings, among them the
    /// microphone's (ADR 0007).
    const CLSID_SETTINGS: GUID = GUID::from_u128(0x087e4db6_0519_4a63_9099_9201915e1371);

    /// The service's general settings interface, in vtable order as in its
    /// type library. rzr uses only the microphone methods at the end.
    #[interface("392375ff-8204-4a26-b4c5-af4b71aca729")]
    unsafe trait IVSSrvSettings: IUnknown {
        fn init(&self, proc_id: u32) -> HRESULT;
        fn get_processing_state(&self, enabled: *mut i32) -> HRESULT;
        fn set_processing_state(&self, enabled: i32) -> HRESULT;
        fn get_hrtf_state(&self, use_custom: *mut i32) -> HRESULT;
        fn set_hrtf_state(&self, use_custom: i32) -> HRESULT;
        fn is_custom_hrtf_present(&self, present: *mut i32) -> HRESULT;
        fn reload_hrtf(&self) -> HRESULT;
        fn get_output_device_params(&self, v: u32, params: *mut u32, data: *mut f32, n: u32) -> HRESULT;
        fn get_default_output_device_params(&self, v: u32, params: *mut u32, data: *mut f32, n: u32) -> HRESULT;
        fn set_output_device_params(&self, v: u32, params: *const u32, data: *const f32, n: u32) -> HRESULT;
        fn get_output_device(&self, id: *mut u16) -> HRESULT;
        fn set_output_device(&self, id: *const u16, kind: u32) -> HRESULT;
        fn get_input_device(&self, id: *mut u16) -> HRESULT;
        fn set_input_device(&self, id: *const u16) -> HRESULT;
        fn get_input_sidetone_state(&self, enabled: *mut i32) -> HRESULT;
        fn set_input_sidetone_state(&self, enabled: i32) -> HRESULT;
        fn get_input_sidetone_level(&self, level: *mut f32) -> HRESULT;
        fn set_input_sidetone_level(&self, level: f32) -> HRESULT;
        fn get_mic_preview_state(&self, enabled: *mut i32) -> HRESULT;
        fn set_mic_preview_state(&self, enabled: i32) -> HRESULT;
        /// `float[10]` in the type library.
        fn get_mic_eq_gains(&self, gains: *mut f32) -> HRESULT;
        fn set_mic_eq_gains(&self, gains: *const f32) -> HRESULT;
        fn get_mic_params(&self, param: u32, value: *mut f32) -> HRESULT;
        fn set_mic_params(&self, param: u32, value: f32) -> HRESULT;
    }

    /// The THX state stored for an output endpoint, if THX is installed on it.
    pub fn read_state(endpoint_id: &str) -> Option<Result<ThxState, String>> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.open_subkey_with_flags(properties_key(endpoint_id)?, KEY_READ).ok()?;
        let json: String = key.get_value(STATE_VALUE).ok()?;
        Some(parse_state(&json))
    }

    /// `GetMicParams` answers 0-17 (pairs of switch and level).
    const MIC_PARAMS: usize = 18;

    /// The microphone enhancements in the service's terms.
    struct MicState {
        eq: [f32; EQ_BANDS],
        params: [f32; MIC_PARAMS],
    }

    impl MicState {
        /// Only the parameters rzr sets are filled in.
        fn from(mic: &MicSettings) -> Self {
            let mut params = [0f32; MIC_PARAMS];
            for effect in MicSettings::EFFECTS {
                let (on, level) = effect.params();
                let fx = mic.effect(effect);
                params[on as usize] = f32::from(u8::from(fx.on));
                params[level as usize] = f32::from(fx.level);
            }
            Self { eq: mic.curve().map(f32::from), params }
        }

        /// This state has everything rzr set in `want`.
        fn shows(&self, want: &MicState) -> bool {
            self.eq == want.eq
                && MicSettings::EFFECTS.iter().all(|e| {
                    let (on, level) = e.params();
                    [on, level].iter().all(|&p| self.params[p as usize] == want.params[p as usize])
                })
        }
    }

    /// Where the service takes ZeroMQ requests, from its discovery key.
    fn zmq_address() -> SocketAddr {
        let url: Option<String> = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(r"SOFTWARE\THX\Discovery", KEY_READ)
            .and_then(|k| k.get_value("thx:sa:service"))
            .ok();
        url.as_deref().and_then(zmq::parse_address).unwrap_or(zmq::DEFAULT_ADDRESS)
    }

    /// The THX service: its COM interfaces, and where it takes ZeroMQ requests.
    pub struct Service {
        com: IVSSrvTHXSettings,
        settings: IVSSrvSettings,
        zmq: SocketAddr,
    }

    impl Service {
        /// Fails if THX isn't installed or its service can't be reached.
        pub fn connect() -> Result<Self, String> {
            let com: IVSSrvTHXSettings = unsafe { CoCreateInstance(&CLSID_THX_SETTINGS, None, CLSCTX_LOCAL_SERVER) }
                .map_err(|e| format!("could not reach the THX service: {e}"))?;
            let settings: IVSSrvSettings = unsafe { CoCreateInstance(&CLSID_SETTINGS, None, CLSCTX_LOCAL_SERVER) }
                .map_err(|e| format!("could not reach the THX service: {e}"))?;
            // The service keeps settings per user session and finds ours by process ID.
            let refused = |e| format!("the THX service refused the connection: {e}");
            unsafe { com.init(std::process::id()) }.ok().map_err(refused)?;
            unsafe { settings.init(std::process::id()) }.ok().map_err(refused)?;
            Ok(Self { com, settings, zmq: zmq_address() })
        }

        /// Make a change and check the service took it: the switches COM
        /// offers go by COM (ADR 0004), the rest by ZeroMQ (ADR 0005).
        pub fn apply(&self, change: ThxChange) -> Result<(), String> {
            match change {
                ThxChange::Switch(option, on) if option != ThxOption::Normalization => {
                    if self.com_set(option, on)? == on {
                        Ok(())
                    } else {
                        Err("the THX service did not accept the change".into())
                    }
                }
                _ => zmq::change(self.zmq, change.patch()),
            }
        }

        /// Pick the THX preset (ZeroMQ) and set its curve (COM), as Synapse
        /// does when a headset preset is chosen (ADR 0006).
        pub fn set_eq(&self, eq: &ThxEq) -> Result<(), String> {
            zmq::set_preset(self.zmq, eq.preset)?;
            let gains = eq.gains();
            let mut sz = 0u64;
            let mut msg = vec![0u8; INBAND_LEN];
            unsafe { self.com.set_current_mode_eq_gains(ORIGINATOR, gains.as_ptr(), &mut sz, msg.as_mut_ptr()) }
                .ok()
                .map_err(|e| format!("the THX service did not apply the curve: {e}"))?;
            // `float[31]` in the type library.
            let mut back = [0f32; 31];
            unsafe { self.com.get_current_mode_eq_gains(back.as_mut_ptr()) }
                .ok()
                .map_err(|e| format!("could not read the THX curve: {e}"))?;
            if back == gains {
                Ok(())
            } else {
                Err("the THX service did not save the curve".into())
            }
        }

        /// The service still answers (a restart breaks the connection, and
        /// the service forgets the microphone settings then).
        pub fn alive(&self) -> bool {
            let mut v = 0i32;
            unsafe { self.settings.get_mic_preview_state(&mut v) }.is_ok()
        }

        /// The microphone enhancements as the service has them.
        fn mic(&self) -> Result<MicState, String> {
            let s = &self.settings;
            let read = |e| format!("could not read the microphone settings from THX: {e}");
            let mut eq = [0f32; EQ_BANDS];
            unsafe { s.get_mic_eq_gains(eq.as_mut_ptr()) }.ok().map_err(read)?;
            let mut params = [0f32; MIC_PARAMS];
            for (p, v) in params.iter_mut().enumerate() {
                unsafe { s.get_mic_params(p as u32, v) }.ok().map_err(read)?;
            }
            Ok(MicState { eq, params })
        }

        /// Send the microphone enhancements that differ, as Synapse does, and
        /// check the service has them all.
        pub fn set_mic(&self, mic: &MicSettings) -> Result<(), String> {
            let s = &self.settings;
            let want = MicState::from(mic);
            let now = self.mic()?;
            let fail = |e| format!("the THX service did not apply the change: {e}");
            let set = |p: u32, v: f32| unsafe { s.set_mic_params(p, v) }.ok().map_err(fail);
            if now.eq != want.eq {
                unsafe { s.set_mic_eq_gains(want.eq.as_ptr()) }.ok().map_err(fail)?;
            }
            for effect in MicSettings::EFFECTS {
                let (on, level) = effect.params();
                let (was_on, want_on) = (now.params[on as usize], want.params[on as usize]);
                let want_level = want.params[level as usize];
                if now.params[level as usize] != want_level {
                    // Synapse switches Voice Clarity off around a level change.
                    let toggle = effect == MicEffect::VoiceClarity && was_on != 0.0;
                    if toggle {
                        set(on, 0.0)?;
                    }
                    set(level, want_level)?;
                    if toggle {
                        set(on, 1.0)?;
                    }
                }
                if was_on != want_on {
                    set(on, want_on)?;
                }
            }
            if self.mic()?.shows(&want) {
                Ok(())
            } else {
                Err("the THX service did not save the microphone enhancements".into())
            }
        }

        /// Switch an option by COM and read it back from the service.
        fn com_set(&self, option: ThxOption, on: bool) -> Result<bool, String> {
            let s = &self.com;
            let mut sz = 0u64;
            let mut msg = vec![0u8; INBAND_LEN];
            let (m, p) = (&mut sz as *mut u64, msg.as_mut_ptr());
            let v = i32::from(on);
            let hr = unsafe {
                match option {
                    ThxOption::Spatial => s.set_spatial_processing_state(ORIGINATOR, v, m, p),
                    ThxOption::BassBoost => s.set_bass_boost_state(ORIGINATOR, v, m, p),
                    ThxOption::VoiceClarity => s.set_dialog_enhance_state(ORIGINATOR, v, m, p),
                    ThxOption::Normalization => return Err("normalization does not go through COM".into()),
                }
            };
            hr.ok().map_err(|e| format!("the THX service did not apply the change: {e}"))?;
            let mut v = 0i32;
            let hr = unsafe {
                match option {
                    ThxOption::Spatial => s.get_spatial_processing_state(&mut v),
                    ThxOption::BassBoost => s.get_bass_boost_state(&mut v),
                    _ => s.get_dialog_enhance_state(&mut v),
                }
            };
            hr.ok().map_err(|e| format!("could not read the THX state: {e}"))?;
            Ok(v != 0)
        }
    }
}

/// Non-Windows builds (development only): no THX.
#[cfg(not(windows))]
mod imp {
    use super::{MicSettings, ThxChange, ThxEq, ThxState};

    pub fn read_state(_endpoint_id: &str) -> Option<Result<ThxState, String>> {
        None
    }

    pub struct Service;

    impl Service {
        pub fn connect() -> Result<Self, String> {
            Err("THX only exists on Windows".into())
        }
        pub fn apply(&self, _change: ThxChange) -> Result<(), String> {
            Err("THX only exists on Windows".into())
        }
        pub fn set_eq(&self, _eq: &ThxEq) -> Result<(), String> {
            Err("THX only exists on Windows".into())
        }
        pub fn set_mic(&self, _mic: &MicSettings) -> Result<(), String> {
            Err("THX only exists on Windows".into())
        }
        pub fn alive(&self) -> bool {
            false
        }
    }
}

#[cfg(not(windows))]
pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-written, shaped like the JSON in docs/RESEARCH.md (not a capture).
    const SAMPLE: &str = r#"{"sequenceNumber":12,"spatialEnabled":false,"hardwareId":"usb\\1532\\0555",
        "outputDevice":"Headphones","presetName":"Music Mode","eqCurve":[0,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
        "tilt":0,"spatialProcessingMode":"Headphones5","drcEnabled":true,"drcLevel":40,
        "emitterPositions":{"L":[30,0,1],"R":[-30,0,1]},"room":{"roomName":"Music Default"},
        "bassBoostEnabled":true,"bassBoost":50,"dialogEnhancementEnabled":false,"dialogEnhancement":100,
        "upmix":{"upmix2p05p1":false}}"#;

    #[test]
    fn parses_the_state_json() {
        let s = parse_state(SAMPLE).unwrap();
        assert_eq!(
            s,
            ThxState {
                sequence_number: 12,
                preset_name: "Music Mode".into(),
                spatial_enabled: false,
                bass_boost_enabled: true,
                bass_boost: 50.0,
                drc_enabled: true,
                drc_level: 40.0,
                dialog_enhancement_enabled: false,
                dialog_enhancement: 100.0,
                eq_curve: [[0.0, 1.0, 1.0, 1.0].as_slice(), &[0.0; 27]].concat(),
            }
        );
    }

    #[test]
    fn missing_fields_default_and_garbage_is_an_error() {
        // The service leaves out nothing today, but a newer one might.
        let s = parse_state(r#"{"sequenceNumber":3,"spatialEnabled":true}"#).unwrap();
        assert!(s.spatial_enabled && !s.bass_boost_enabled && s.preset_name.is_empty());
        assert!(parse_state("").is_err());
        assert!(parse_state("{\"spatialEnabled\":\"yes\"}").is_err());
    }

    #[test]
    fn options_read_and_write_their_own_field() {
        let mut s = ThxState::default();
        for option in [ThxOption::Spatial, ThxOption::BassBoost, ThxOption::VoiceClarity] {
            let change = ThxChange::Switch(option, true);
            assert!(!change.shown_in(&s));
            change.apply(&mut s);
            assert!(change.shown_in(&s) && option.is_on(&s));
        }
        assert!(s.spatial_enabled && s.bass_boost_enabled && s.dialog_enhancement_enabled && !s.drc_enabled);
        ThxChange::Switch(ThxOption::Normalization, true).apply(&mut s);
        assert!(s.drc_enabled);
    }

    #[test]
    fn levels_read_and_write_their_own_field() {
        let mut s = ThxState::default();
        ThxChange::Level(ThxLevel::BassBoost, 30.0).apply(&mut s);
        ThxChange::Level(ThxLevel::Normalization, 40.0).apply(&mut s);
        ThxChange::Level(ThxLevel::VoiceClarity, 50.0).apply(&mut s);
        assert_eq!((s.bass_boost, s.drc_level, s.dialog_enhancement), (30.0, 40.0, 50.0));
        assert!(!s.bass_boost_enabled, "a level doesn't switch the option on");
        // The service stores doubles; the page sends whole numbers.
        s.bass_boost = 30.2;
        assert!(ThxChange::Level(ThxLevel::BassBoost, 30.0).shown_in(&s));
        assert!(!ThxChange::Level(ThxLevel::BassBoost, 31.0).shown_in(&s));
        assert_eq!(ThxChange::Level(ThxLevel::Normalization, 0.0).label(), "Sound Normalization level");
    }

    #[test]
    fn changes_patch_the_state_field_they_show() {
        use proto::{state, Patch};
        let cases = [
            (ThxChange::Switch(ThxOption::Normalization, true), Patch::Bool(state::DRC_ENABLED, true)),
            (ThxChange::Switch(ThxOption::BassBoost, false), Patch::Bool(state::BASS_BOOST_ENABLED, false)),
            (ThxChange::Level(ThxLevel::BassBoost, 70.0), Patch::Double(state::BASS_BOOST, 70.0)),
            (ThxChange::Level(ThxLevel::Normalization, 20.0), Patch::Double(state::DRC_LEVEL, 20.0)),
            (ThxChange::Level(ThxLevel::VoiceClarity, 0.0), Patch::Double(state::DIALOG_ENHANCEMENT, 0.0)),
        ];
        for (change, patch) in cases {
            assert_eq!(change.patch(), patch);
        }
    }

    #[test]
    fn zmq_address_from_the_discovery_key() {
        assert_eq!(zmq::parse_address("tcp://127.0.0.1:49671"), Some(zmq::DEFAULT_ADDRESS));
        assert_eq!(zmq::parse_address(" tcp://127.0.0.1:50000 ").map(|a| a.port()), Some(50000));
        // Only this PC: rzr never sends the THX state anywhere else.
        assert_eq!(zmq::parse_address("tcp://192.168.1.5:49671"), None);
        assert_eq!(zmq::parse_address("tcp://*:49671"), None);
        assert_eq!(zmq::parse_address("127.0.0.1:49671"), None);
    }

    #[test]
    fn properties_key_from_endpoint_id() {
        assert_eq!(
            properties_key("{0.0.0.00000000}.{68854153-5b36-4efa-b61a-6ffb850dd6fa}").as_deref(),
            Some(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{68854153-5b36-4efa-b61a-6ffb850dd6fa}\Properties"
            )
        );
        // Inputs, demo IDs and anything that could climb out of the key are refused.
        assert_eq!(properties_key("{0.0.1.00000000}.{68854153-5b36-4efa-b61a-6ffb850dd6fa}"), None);
        assert_eq!(properties_key("demo-out"), None);
        assert_eq!(properties_key(r"{0.0.0.00000000}.{..\..\..\..\..\..\..\..\..\..\..\}"), None);
    }

    #[test]
    fn thx_eq_like_synapse() {
        let game = ThxEq::new(EqPreset::Game, EqPreset::Game.curve().unwrap());
        assert_eq!(game.preset, "Game Mode");
        assert_eq!(ThxEq::new(EqPreset::Movie, [0; 10]).preset, "Cinema Mode");
        assert_eq!(ThxEq::new(EqPreset::Music, [0; 10]).preset, "Music Mode");
        assert_eq!(ThxEq::new(EqPreset::Custom, [0; 10]).preset, "Custom");
        assert_eq!(ThxEq::new(EqPreset::Csgo, [0; 10]).preset, "Custom");
        // Music as the service stores it (docs/RESEARCH.md, `eqCurve`).
        let music = ThxEq::new(EqPreset::Music, EqPreset::Music.curve().unwrap());
        let stored = [0, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 1, 1, 1, 0, 0, 0];
        assert_eq!(music.gains(), stored.map(|v: i8| f32::from(v)));

        let mut s = parse_state(SAMPLE).unwrap();
        assert!(!music.shown_in(&s), "same preset, other curve");
        music.apply(&mut s);
        assert!(music.shown_in(&s) && !game.shown_in(&s));
    }

    #[test]
    fn preset_labels() {
        assert_eq!(preset_label("Cinema Mode"), "Movie");
        assert_eq!(preset_label("Custom"), "Custom");
        assert_eq!(preset_label("Something New"), "Something New");
    }
}
