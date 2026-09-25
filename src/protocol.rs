/// Audio MXIC ("PA") protocol for the Razer BlackShark V2 Pro 2.4 dongle (1532:0555).
///
/// Every command is a 64-byte HID report (report id 0x02):
///
/// ```text
///   [0]  0x02       report id
///   [1]  0x80       direction (host -> device)
///   [2]  total_len  8 + payload length
///   [5]  0x50 'P'
///   [6]  0x41 'A'
///   [7]  inner_len  0x08 (0x0E for the remote-mode frame)
///   [9]  cmd_type   0x02 remote, 0x03 query, 0x04 audio write, 0x0D EQ write
///   [10] cmd_id
///   [11] flag       0 in a request
///   [12] data_len
///   [13] data...
/// ```
///
/// Replies repeat the sub-frame shifted by two: [12]=cmd id, [13]=0x01 ACK,
/// [14]=length, [15..]=payload.
///
/// Command table cross-checked against the OpenRazer driver for this exact
/// device (openrazer/openrazer#2862), which verified each register on hardware.

pub const PKT_SIZE: usize = 64;
pub type Packet = [u8; PKT_SIZE];

const REPORT_ID: u8 = 0x02;

// Command types (byte 9)
pub const TYPE_REMOTE: u8 = 0x02;
pub const TYPE_QUERY: u8 = 0x03;
pub const TYPE_AUDIO: u8 = 0x04;
pub const TYPE_CONFIG: u8 = 0x06;
pub const TYPE_EQ: u8 = 0x0D;

// Getters (type 0x03). Rule: get_id = set_id - 0x80.
pub const CMD_SERIAL: u8 = 0x00;
pub const CMD_FIRMWARE: u8 = 0x02;
pub const CMD_PRESET_GET: u8 = 0x13;
#[allow(dead_code)] // documented, not read yet
pub const CMD_EQ_GET: u8 = 0x15;
#[allow(dead_code)] // documented, not read yet
pub const CMD_SIDETONE_GET: u8 = 0x18;
#[allow(dead_code)] // documented, not read yet
pub const CMD_SIDETONE_LEVEL_GET: u8 = 0x19;
pub const CMD_PREP: u8 = 0x1E;
pub const CMD_WIRELESS: u8 = 0x20;
pub const CMD_BATTERY: u8 = 0x21;
#[allow(dead_code)] // documented, not read yet
pub const CMD_DND_GET: u8 = 0x27;
pub const CMD_CHARGING: u8 = 0x2A;
#[allow(dead_code)] // documented, not read yet
pub const CMD_AUTO_OFF_GET: u8 = 0x2C;
pub const CMD_MIC_MUTE: u8 = 0x55;

// Setters
pub const CMD_CONFIG: u8 = 0x01; // type 0x06, sent by Synapse on profile switch
pub const CMD_PRESET: u8 = 0x93; // EQ preset selector (see EqPreset)
pub const CMD_EQ: u8 = 0x95; // 10 signed dB values, type 0x0D
pub const CMD_SIDETONE: u8 = 0x98; // mic monitoring on/off
pub const CMD_SIDETONE_LEVEL: u8 = 0x99; // mic monitoring level 1-10
pub const CMD_MODE_FLAG: u8 = 0x9D; // 1 = classic preset, 2 = esports preset
pub const CMD_DND: u8 = 0xA7; // Bluetooth "Do Not Disturb"
pub const CMD_AUTO_OFF: u8 = 0xAC; // auto power-off minutes, 0 = off
pub const CMD_REMOTE: u8 = 0xE1;

pub const EQ_BANDS: usize = 10;
pub const EQ_FREQS: [&str; EQ_BANDS] = [
    "31Hz", "63Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz",
];
/// Range Synapse offers for this headset's EQ.
pub const EQ_MIN_DB: i8 = -5;
pub const EQ_MAX_DB: i8 = 5;

pub const SIDETONE_MAX: u8 = 10;
pub const AUTO_OFF_MIN_MINUTES: u8 = 15;
pub const AUTO_OFF_MAX_MINUTES: u8 = 60;

/// Raw MXIC frame: envelope plus bytes 10.. as given.
fn frame(total_len: u8, inner_len: u8, cmd_type: u8, cmd_id: u8, rest: &[u8]) -> Packet {
    let mut pkt = [0u8; PKT_SIZE];
    pkt[0] = REPORT_ID;
    pkt[1] = 0x80; // Direction: output
    pkt[2] = total_len;
    pkt[5] = 0x50; // 'P'
    pkt[6] = 0x41; // 'A'
    pkt[7] = inner_len;
    pkt[9] = cmd_type;
    pkt[10] = cmd_id;
    pkt[11..11 + rest.len()].copy_from_slice(rest);
    pkt
}

/// Standard command: flag 0, length-prefixed payload.
pub fn command(cmd_type: u8, cmd_id: u8, data: &[u8]) -> Packet {
    let mut rest = Vec::with_capacity(2 + data.len());
    rest.push(0x00);
    rest.push(data.len() as u8);
    rest.extend_from_slice(data);
    frame(8 + data.len() as u8, 0x08, cmd_type, cmd_id, &rest)
}

/// Query a register (type 0x03, no payload).
pub fn query(cmd_id: u8) -> Packet {
    command(TYPE_QUERY, cmd_id, &[])
}

/// Set Remote/Local mode. Remote=true gives software control, false returns
/// control to the headset. The parameter rides in the flag byte.
pub fn set_remote_mode(enable: bool) -> Packet {
    frame(0x07, 0x0E, TYPE_REMOTE, CMD_REMOTE, &[enable as u8])
}

/// SET_CONFIG (0x06/0x01), captured from Synapse's profile switch.
/// Params C2 03 F8 5F 04 (meaning unknown; kept for parity with Synapse).
pub fn set_config() -> Packet {
    frame(0x0B, 0x08, TYPE_CONFIG, CMD_CONFIG, &[0xC2, 0x03, 0xF8, 0x5F, 0x04])
}

/// Select an EQ preset (0x93).
pub fn set_preset(preset: EqPreset) -> Packet {
    command(TYPE_AUDIO, CMD_PRESET, &[preset.selector()])
}

/// Preset family flag (0x9D) that Synapse sends after every preset select.
pub fn set_mode_flag(preset: EqPreset) -> Packet {
    command(TYPE_AUDIO, CMD_MODE_FLAG, &[if preset.is_esports() { 2 } else { 1 }])
}

/// Set the 10-band custom EQ (0x0D/0x95). Signed dB per band.
pub fn set_eq_bands(bands: &[i8; EQ_BANDS]) -> Packet {
    let data: Vec<u8> = bands.iter().map(|&b| b as u8).collect();
    command(TYPE_EQ, CMD_EQ, &data)
}

/// Single-byte audio setting (sidetone, DND, auto power-off).
pub fn set_value(cmd_id: u8, value: u8) -> Packet {
    command(TYPE_AUDIO, cmd_id, &[value])
}

/// Extract the payload of a reply to `cmd_id`, if `buf` is one.
/// Replies carry the echoed id at [12] and 0x01 (ACK) at [13]; the ACK check
/// filters out the unsolicited telemetry frames the headset also sends.
pub fn parse_reply(buf: &[u8], cmd_id: u8) -> Option<&[u8]> {
    // hidapi keeps the report id at [0]; tolerate a backend that strips it.
    let off = if buf.first() == Some(&REPORT_ID) { 12 } else { 11 };
    if buf.len() <= off + 2 || buf[off] != cmd_id || buf[off + 1] != 0x01 {
        return None;
    }
    let start = off + 3;
    let end = (start + buf[off + 2] as usize).min(buf.len());
    Some(&buf[start..end])
}

/// EQ presets. Game/Music/Movie and the five esports presets are stored on the
/// headset and activated by selector alone; Custom plays the host-written curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqPreset {
    Game,
    Movie,
    Music,
    Custom,
    ApexLegends,
    CallOfDuty,
    Csgo,
    Fortnite,
    Valorant,
}

impl EqPreset {
    pub const STANDARD: [EqPreset; 4] = [Self::Game, Self::Movie, Self::Music, Self::Custom];
    pub const ESPORTS: [EqPreset; 5] = [
        Self::ApexLegends,
        Self::CallOfDuty,
        Self::Csgo,
        Self::Fortnite,
        Self::Valorant,
    ];

    /// Selector byte for cmd 0x93. Esports ids follow Synapse's list order.
    pub fn selector(self) -> u8 {
        match self {
            Self::Game => 0x07,
            Self::Music => 0x08,
            Self::Movie => 0x09,
            Self::Custom => 0xFF,
            Self::ApexLegends => 0xFA,
            Self::CallOfDuty => 0xFE,
            Self::Csgo => 0xFB,
            Self::Fortnite => 0xFD,
            Self::Valorant => 0xFC,
        }
    }

    pub fn from_selector(sel: u8) -> Option<Self> {
        Self::STANDARD
            .iter()
            .chain(Self::ESPORTS.iter())
            .copied()
            .find(|p| p.selector() == sel)
    }

    pub fn is_esports(self) -> bool {
        Self::ESPORTS.contains(&self)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Game => "JUEGO",
            Self::Movie => "PELÍCULA",
            Self::Music => "MÚSICA",
            Self::Custom => "PERSONALIZADO",
            Self::ApexLegends => "APEX LEGENDS",
            Self::CallOfDuty => "CALL OF DUTY",
            Self::Csgo => "CS2",
            Self::Fortnite => "FORTNITE",
            Self::Valorant => "VALORANT",
        }
    }

    /// Reference curve for display (Synapse 4 defaults for this headset).
    /// The headset plays its own stored copy; Custom has no fixed curve.
    pub fn curve(self) -> Option<[i8; EQ_BANDS]> {
        Some(match self {
            Self::Game => [-5, -5, -4, -3, -2, 2, 4, 4, 3, 2],
            Self::Movie => [4, 4, 3, -4, -5, -1, 3, 5, 3, 2],
            Self::Music => [1, 1, 1, 0, 0, 2, 3, 3, 2, 1],
            Self::ApexLegends => [-1, 0, 0, 0, -1, 0, 2, 3, 2, 3],
            Self::CallOfDuty => [2, 0, 5, 0, 0, 0, -4, -4, -4, -4],
            Self::Csgo => [-5, -4, 3, 5, 3, -2, 1, 5, 4, -4],
            Self::Fortnite => [-4, 5, 5, 3, -2, 3, 4, 4, -1, 4],
            Self::Valorant => [0, 0, 0, 0, 0, 1, 4, 4, 4, -3],
            Self::Custom => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte-for-byte copies of the frames the original rzr release sent (and
    /// which are known to work on hardware).
    fn legacy(total_len: u8, inner_len: u8, cmd_type: u8, cmd_id: u8, params: &[u8]) -> Packet {
        let mut pkt = [0u8; PKT_SIZE];
        pkt[0] = 0x02;
        pkt[1] = 0x80;
        pkt[2] = total_len;
        pkt[5] = 0x50;
        pkt[6] = 0x41;
        pkt[7] = inner_len;
        pkt[9] = cmd_type;
        pkt[10] = cmd_id;
        for (i, &b) in params.iter().enumerate() {
            pkt[11 + i] = b;
        }
        pkt
    }

    #[test]
    fn matches_legacy_frames() {
        assert_eq!(set_remote_mode(true), legacy(0x07, 0x0E, 0x02, 0xE1, &[1]));
        assert_eq!(set_remote_mode(false), legacy(0x07, 0x0E, 0x02, 0xE1, &[0]));
        assert_eq!(set_config(), legacy(0x0B, 0x08, 0x06, 0x01, &[0xC2, 0x03, 0xF8, 0x5F, 0x04]));
        // Legacy "setVolume(255)" is really the Custom preset selector.
        assert_eq!(set_preset(EqPreset::Custom), legacy(0x09, 0x08, 0x04, 0x93, &[0x00, 0x01, 0xFF]));
        // Legacy "setEnhancement(true)" is the classic-family flag.
        assert_eq!(set_mode_flag(EqPreset::Custom), legacy(0x09, 0x08, 0x04, 0x9D, &[0x00, 0x01, 0x01]));

        let bands: [i8; 10] = [1, -2, 1, -3, 1, -3, -5, 2, 2, 3];
        let mut params = vec![0x00, 0x0A];
        params.extend(bands.iter().map(|&b| b as u8));
        assert_eq!(set_eq_bands(&bands), legacy(0x12, 0x08, 0x0D, 0x95, &params));

        assert_eq!(query(CMD_BATTERY), legacy(0x08, 0x08, 0x03, 0x21, &[]));
    }

    #[test]
    fn parses_replies() {
        let mut buf = [0u8; 64];
        buf[0] = 0x02;
        buf[12] = CMD_BATTERY;
        buf[13] = 0x01;
        buf[14] = 1;
        buf[15] = 70;
        assert_eq!(parse_reply(&buf, CMD_BATTERY), Some(&[70u8][..]));
        assert_eq!(parse_reply(&buf, CMD_CHARGING), None);
        buf[13] = 0x00; // not an ACK: telemetry
        assert_eq!(parse_reply(&buf, CMD_BATTERY), None);
    }

    #[test]
    fn preset_selectors_round_trip() {
        for p in EqPreset::STANDARD.iter().chain(EqPreset::ESPORTS.iter()) {
            assert_eq!(EqPreset::from_selector(p.selector()), Some(*p));
        }
        assert_eq!(EqPreset::from_selector(0x00), None);
    }
}
