//! THX Spatial Audio: reading its state and switching its options.
//!
//! THX is an audio effect in Windows, not part of the headset (see
//! docs/HALLAZGOS.md). Its full state is a JSON string that the THX service
//! (`VSSrv.exe`) keeps in the headset's output endpoint properties; rzr reads
//! it from the registry, because the endpoint's property store cuts strings at
//! 259 characters and the JSON is about 1 KB. Changes go through the service's
//! COM interface (`IVSSrvTHXSettings`), the same service Synapse talks to,
//! which then rewrites the JSON. Only the on/off switches that interface
//! offers are used: THX Spatial Audio, Bass Boost and Voice Clarity (ADR 0004).

use serde::Deserialize;

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
}

/// Parse the JSON the THX service stores for the endpoint.
pub fn parse_state(json: &str) -> Result<ThxState, String> {
    serde_json::from_str(json).map_err(|e| format!("estado de THX ilegible: {e}"))
}

/// A THX option rzr can switch on and off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThxOption {
    Spatial,
    BassBoost,
    VoiceClarity,
}

impl ThxOption {
    pub fn is_on(self, s: &ThxState) -> bool {
        match self {
            ThxOption::Spatial => s.spatial_enabled,
            ThxOption::BassBoost => s.bass_boost_enabled,
            ThxOption::VoiceClarity => s.dialog_enhancement_enabled,
        }
    }

    pub fn set(self, s: &mut ThxState, on: bool) {
        match self {
            ThxOption::Spatial => s.spatial_enabled = on,
            ThxOption::BassBoost => s.bass_boost_enabled = on,
            ThxOption::VoiceClarity => s.dialog_enhancement_enabled = on,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThxOption::Spatial => "THX Spatial Audio",
            ThxOption::BassBoost => "Bass Boost",
            ThxOption::VoiceClarity => "Claridad de voz",
        }
    }
}

/// The THX preset's name as the page shows it.
pub fn preset_label(name: &str) -> &str {
    match name {
        "Game Mode" => "Juego",
        "Cinema Mode" => "Película",
        "Music Mode" => "Música",
        "Custom" => "Personalizado",
        other => other,
    }
}

#[cfg(windows)]
pub use imp::*;

#[cfg(windows)]
mod imp {
    use super::{parse_state, properties_key, ThxOption, ThxState};
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

    /// The THX state stored for an output endpoint, if THX is installed on it.
    pub fn read_state(endpoint_id: &str) -> Option<Result<ThxState, String>> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = hklm.open_subkey_with_flags(properties_key(endpoint_id)?, KEY_READ).ok()?;
        let json: String = key.get_value(STATE_VALUE).ok()?;
        Some(parse_state(&json))
    }

    /// A connection to the THX service.
    pub struct Service(IVSSrvTHXSettings);

    impl Service {
        /// Fails if THX isn't installed or its service can't be reached.
        pub fn connect() -> Result<Self, String> {
            let settings: IVSSrvTHXSettings =
                unsafe { CoCreateInstance(&CLSID_THX_SETTINGS, None, CLSCTX_LOCAL_SERVER) }
                    .map_err(|e| format!("no se pudo conectar con el servicio de THX: {e}"))?;
            // The service keeps settings per user session and finds ours by process ID.
            unsafe { settings.init(std::process::id()) }
                .ok()
                .map_err(|e| format!("el servicio de THX rechazó la conexión: {e}"))?;
            Ok(Self(settings))
        }

        /// Switch an option and read it back from the service.
        pub fn set(&self, option: ThxOption, on: bool) -> Result<bool, String> {
            let s = &self.0;
            let mut sz = 0u64;
            let mut msg = vec![0u8; INBAND_LEN];
            let (m, p) = (&mut sz as *mut u64, msg.as_mut_ptr());
            let v = i32::from(on);
            let hr = unsafe {
                match option {
                    ThxOption::Spatial => s.set_spatial_processing_state(ORIGINATOR, v, m, p),
                    ThxOption::BassBoost => s.set_bass_boost_state(ORIGINATOR, v, m, p),
                    ThxOption::VoiceClarity => s.set_dialog_enhance_state(ORIGINATOR, v, m, p),
                }
            };
            hr.ok().map_err(|e| format!("el servicio de THX no aplicó el cambio: {e}"))?;
            self.get(option)
        }

        fn get(&self, option: ThxOption) -> Result<bool, String> {
            let s = &self.0;
            let mut v = 0i32;
            let hr = unsafe {
                match option {
                    ThxOption::Spatial => s.get_spatial_processing_state(&mut v),
                    ThxOption::BassBoost => s.get_bass_boost_state(&mut v),
                    ThxOption::VoiceClarity => s.get_dialog_enhance_state(&mut v),
                }
            };
            hr.ok().map_err(|e| format!("no se pudo leer el estado de THX: {e}"))?;
            Ok(v != 0)
        }
    }
}

/// Non-Windows builds (development only): no THX.
#[cfg(not(windows))]
mod imp {
    use super::{ThxOption, ThxState};

    pub fn read_state(_endpoint_id: &str) -> Option<Result<ThxState, String>> {
        None
    }

    pub struct Service;

    impl Service {
        pub fn connect() -> Result<Self, String> {
            Err("THX solo existe en Windows".into())
        }
        pub fn set(&self, _option: ThxOption, _on: bool) -> Result<bool, String> {
            Err("THX solo existe en Windows".into())
        }
    }
}

#[cfg(not(windows))]
pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-written, shaped like the JSON in docs/HALLAZGOS.md (not a capture).
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
            assert!(!option.is_on(&s));
            option.set(&mut s, true);
            assert!(option.is_on(&s));
        }
        assert!(s.spatial_enabled && s.bass_boost_enabled && s.dialog_enhancement_enabled && !s.drc_enabled);
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
    fn preset_labels() {
        assert_eq!(preset_label("Cinema Mode"), "Película");
        assert_eq!(preset_label("Custom"), "Personalizado");
        assert_eq!(preset_label("Something New"), "Something New");
    }
}
