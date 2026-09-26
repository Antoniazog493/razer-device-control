//! Audio MXIC ("PA") protocol for the Razer BlackShark V2 Pro (2023) dongle (1532:0555).
//!
//! Every command is a 64-byte HID report (report id 0x02):
//!
//! ```text
//!   [0]  0x02       report id
//!   [1]  0x80       direction (host -> device)
//!   [2]  total_len  8 + payload length
//!   [5]  0x50 'P'
//!   [6]  0x41 'A'
//!   [7]  inner_len  0x08 (0x0E for the remote-mode frame)
//!   [9]  cmd_type   0x02 remote, 0x03 query, 0x04 audio write, 0x0D EQ write
//!   [10] cmd_id
//!   [11] flag       0 in a request
//!   [12] data_len
//!   [13] data...
//! ```
//!
//! Replies are one or more "PI" frames packed after [0]=report id and
//! [1]=total length. Each frame is 'P' 'I', eight header bytes, then
//! cmd id, flag (0x01 = ACK of a request, 0x02 = unsolicited event), data
//! length and data. The first frame puts the cmd id at [12].
//!
//! Command table cross-checked against the OpenRazer driver for this exact
//! device (openrazer/openrazer#2862), which verified each register on
//! hardware, and against Synapse 4's own logs (names in quotes below).

pub const PKT_SIZE: usize = 64;
pub type Packet = [u8; PKT_SIZE];

const REPORT_ID: u8 = 0x02;

// Command types (byte 9)
pub const TYPE_REMOTE: u8 = 0x02;
pub const TYPE_QUERY: u8 = 0x03;
pub const TYPE_AUDIO: u8 = 0x04;
pub const TYPE_DONGLE: u8 = 0x06;
pub const TYPE_EQ: u8 = 0x0D;

// Getters (type 0x03). Rule: get_id = set_id - 0x80.
pub const CMD_SERIAL: u8 = 0x00;
pub const CMD_FIRMWARE: u8 = 0x02;
pub const CMD_PRESET_GET: u8 = 0x13;
pub const CMD_EQ_GET: u8 = 0x15;
#[allow(dead_code)] // documented, not read yet
pub const CMD_SIDETONE_GET: u8 = 0x18;
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
pub const CMD_DONGLE_FIRMWARE: u8 = 0x01; // type 0x06, "Get Dongle Firmware Version"
pub const CMD_PRESET: u8 = 0x93; // "Set Preset EQ Index" (see EqPreset)
pub const CMD_EQ: u8 = 0x95; // "Set Customer EQ Band": 10 signed dB values, type 0x0D
pub const CMD_MIC_EQ_PRESET: u8 = 0x96; // "Set Mic Preset EQ Index" (see mic::MicEqPreset)
pub const CMD_SIDETONE: u8 = 0x98; // "Set Sidetone status"
pub const CMD_SIDETONE_LEVEL: u8 = 0x99; // "Set Sidetone Volume"
pub const CMD_MODE_FLAG: u8 = 0x9D; // "Set Speaker Preset EQ Group": 1 classic, 2 esports
pub const CMD_PRESET_EQ_STATUS: u8 = 0x9E; // "Set Speaker Preset EQ Status": Synapse sends 0 at start
pub const CMD_DND: u8 = 0xA7; // Bluetooth "Do Not Disturb"
pub const CMD_AUTO_OFF: u8 = 0xAC; // auto power-off minutes, 0 = off
pub const CMD_REMOTE: u8 = 0xE1;

pub const EQ_BANDS: usize = 10;
pub const EQ_FREQS: [&str; EQ_BANDS] =
    ["31Hz", "63Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz"];
/// Range Synapse offers for this headset's EQ.
pub const EQ_MIN_DB: i8 = -5;
pub const EQ_MAX_DB: i8 = 5;

/// Highest sidetone level sent. Synapse maps its 0-100 slider linearly
/// (50 -> 7 and 31 -> 4 on the wire), so 100 -> 14.
pub const SIDETONE_MAX: u8 = 14;
/// Highest level OpenRazer saw the headset accept; used as a fallback if it
/// rejects a higher one.
pub const SIDETONE_SAFE_MAX: u8 = 10;
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

/// "Get Dongle Firmware Version" (0x06/0x01), the first frame of Synapse's
/// startup. Addressed to the dongle itself; the reply carries flag 0xC2 and
/// four version bytes. (Earlier rzr releases called this SET_CONFIG.)
pub fn get_dongle_firmware() -> Packet {
    frame(0x0B, 0x08, TYPE_DONGLE, CMD_DONGLE_FIRMWARE, &[0xC2, 0x03, 0xF8, 0x5F, 0x04])
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

/// One "PI" frame of an input report.
pub struct Frame<'a> {
    pub cmd: u8,
    pub flag: u8,
    pub data: &'a [u8],
}

pub const FLAG_ACK: u8 = 0x01;
/// Unsolicited event (link, battery, mic mute, DND...).
pub const FLAG_EVENT: u8 = 0x02;

/// Split an input report into its "PI" frames.
pub fn frames(buf: &[u8]) -> Vec<Frame<'_>> {
    // hidapi keeps the report id at [0]; tolerate a backend that strips it.
    let (mut off, end) = if buf.first() == Some(&REPORT_ID) && buf.len() > 1 {
        (2, (2 + buf[1] as usize).min(buf.len()))
    } else {
        (1, buf.len())
    };
    // Each frame: "PI", 6 header bytes, a little-endian body size, then the
    // body: cmd, flag and usually a data length and the data. Some bodies
    // are just cmd and flag (the dongle's E3 link notice), so the frame is
    // walked by its body size, never by the data length.
    let mut out = Vec::new();
    while off + 12 <= end && buf[off] == b'P' && buf[off + 1] == b'I' {
        let size = u16::from_le_bytes([buf[off + 8], buf[off + 9]]) as usize;
        let body = &buf[off + 10..(off + 10 + size).min(end)];
        if body.len() >= 2 {
            let data = body.get(3..).unwrap_or(&[]);
            let len = body.get(2).map_or(0, |&l| l as usize).min(data.len());
            out.push(Frame { cmd: body[0], flag: body[1], data: &data[..len] });
        }
        off += 10 + size;
    }
    out
}

/// Payload of the ACK to `cmd_id`, if `buf` carries one. Matching the ACK
/// flag skips the unsolicited event frames the headset also sends.
pub fn parse_reply(buf: &[u8], cmd_id: u8) -> Option<&[u8]> {
    frames(buf).into_iter().find(|f| f.cmd == cmd_id && f.flag == FLAG_ACK).map(|f| f.data)
}

/// Dongle firmware version ("2.4.1.0") from the reply to get_dongle_firmware().
pub fn parse_dongle_firmware(buf: &[u8]) -> Option<String> {
    let f = frames(buf).into_iter().find(|f| f.cmd == CMD_DONGLE_FIRMWARE && f.flag == 0xC2 && f.data.len() >= 4)?;
    Some(f.data[..4].iter().map(|b| b.to_string()).collect::<Vec<_>>().join("."))
}

/// Wire level for Synapse's 0-100 sidetone slider.
pub fn sidetone_level(percent: u8) -> u8 {
    ((percent.min(100) as u16 * SIDETONE_MAX as u16 + 50) / 100) as u8
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
    pub const ESPORTS: [EqPreset; 5] =
        [Self::ApexLegends, Self::CallOfDuty, Self::Csgo, Self::Fortnite, Self::Valorant];

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
        Self::STANDARD.iter().chain(Self::ESPORTS.iter()).copied().find(|p| p.selector() == sel)
    }

    pub fn is_esports(self) -> bool {
        Self::ESPORTS.contains(&self)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Game => "Game",
            Self::Movie => "Movie",
            Self::Music => "Music",
            Self::Custom => "Custom",
            Self::ApexLegends => "Apex Legends",
            Self::CallOfDuty => "Call of Duty",
            Self::Csgo => "CS2",
            Self::Fortnite => "Fortnite",
            Self::Valorant => "Valorant",
        }
    }

    /// Preset curve. Esports curves are the ones Synapse writes into each
    /// preset's slot; Game/Music/Movie are reference curves for display (the
    /// headset plays its built-in copy; Movie matches Synapse's own).
    /// Custom has no fixed curve.
    pub fn curve(self) -> Option<[i8; EQ_BANDS]> {
        Some(match self {
            Self::Game => [-3, -3, -4, 0, 5, 5, 4, 1, 0, -1],
            Self::Movie => [4, 4, 3, 0, -3, -1, 3, 5, 2, 1],
            Self::Music => [2, 2, 1, 1, 2, 3, 3, 3, 1, 0],
            Self::ApexLegends => [-1, 0, 0, 0, -1, 0, 2, 3, 2, 3],
            Self::CallOfDuty => [-2, 0, 3, 3, 3, 3, 0, 0, 0, 0],
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
        assert_eq!(get_dongle_firmware(), legacy(0x0B, 0x08, 0x06, 0x01, &[0xC2, 0x03, 0xF8, 0x5F, 0x04]));
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
    fn mic_eq_preset_matches_synapse() {
        // Synapse's "Set Mic Preset EQ Index" for Conference (3) and Custom (255),
        // from its log of this headset (2026-09-26): same shape as 0x98.
        assert_eq!(set_value(CMD_MIC_EQ_PRESET, 3), legacy(0x09, 0x08, 0x04, 0x96, &[0x00, 0x01, 0x03]));
        assert_eq!(set_value(CMD_MIC_EQ_PRESET, 255), legacy(0x09, 0x08, 0x04, 0x96, &[0x00, 0x01, 0xFF]));
    }

    /// Pad a captured report to 64 bytes.
    fn report(bytes: &[u8]) -> Vec<u8> {
        let mut v = bytes.to_vec();
        v.resize(64, 0);
        v
    }

    // Input reports copied from a Synapse 4 log of this headset.

    #[test]
    fn parses_captured_replies() {
        // getBatteryStatus -> 55%. Trailing bytes are stale buffer contents.
        let battery = report(&[
            0x02, 0x0E, 0x50, 0x49, 0x08, 0xD5, 0xCD, 0x1D, 0x00, 0x00, 0x04, 0x00, 0x21, 0x01, 0x01, 0x37, 0x49, 0x01,
            0xC0, 0x22,
        ]);
        assert_eq!(parse_reply(&battery, CMD_BATTERY), Some(&[55u8][..]));
        assert_eq!(parse_reply(&battery, CMD_CHARGING), None);

        // The transport-level echo that precedes each reply is not an ACK.
        let echo = report(&[0x02, 0x0D, 0x50, 0x49, 0x01, 0xC0, 0xFE, 0x5B, 0xEF, 0x00, 0x03, 0x00, 0x0E, 0x80]);
        assert!(frames(&echo).iter().all(|f| f.flag != FLAG_ACK));

        // Set Not Disturb ACK packed together with a DND-changed event.
        let dnd = report(&[
            2, 28, 80, 73, 8, 212, 12, 125, 3, 0, 4, 0, 167, 1, 1, 0, 80, 73, 8, 213, 12, 125, 3, 0, 4, 0, 39, 2, 1, 1,
        ]);
        let f = frames(&dnd);
        assert_eq!(f.len(), 2);
        assert_eq!((f[1].cmd, f[1].flag, f[1].data), (CMD_DND_GET, FLAG_EVENT, &[1u8][..]));
        assert_eq!(parse_reply(&dnd, CMD_DND), Some(&[0u8][..]));
        assert_eq!(parse_reply(&dnd, CMD_DND_GET), None); // event, not an ACK

        // Get Dongle Firmware Version -> 2.4.1.0
        let fw = report(&[
            0x02, 0x11, 0x50, 0x49, 0x08, 0xD6, 0x87, 0x5D, 0xEF, 0x00, 0x07, 0x00, 0x01, 0xC2, 0x04, 0x02, 0x04, 0x01,
            0x00,
        ]);
        assert_eq!(parse_dongle_firmware(&fw).as_deref(), Some("2.4.1.0"));
    }

    #[test]
    fn link_drop_event_after_a_short_frame() {
        // Captured by rzr when the link dropped: the dongle's two-byte E3
        // notice, then the "link down" event. Reading E3's body as having a
        // length byte used to swallow the event.
        let drop = report(&[
            0x02, 0x1A, 0x50, 0x49, 0x0E, 0xC4, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0xE3, 0x00, 0x50, 0x49, 0x08, 0xC5,
            0x59, 0x28, 0x15, 0x00, 0x04, 0x00, 0x20, 0x02, 0x01, 0x00,
        ]);
        let f = frames(&drop);
        assert_eq!(f.len(), 2);
        assert_eq!((f[0].cmd, f[0].flag, f[0].data), (0xE3, 0x00, &[][..]));
        assert_eq!((f[1].cmd, f[1].flag, f[1].data), (CMD_WIRELESS, FLAG_EVENT, &[0u8][..]));
    }

    #[test]
    fn sidetone_matches_synapse() {
        // Synapse sent 7 for 50 and 4 for 31.
        assert_eq!(sidetone_level(50), 7);
        assert_eq!(sidetone_level(31), 4);
        assert_eq!(sidetone_level(100), SIDETONE_MAX);
        assert_eq!(sidetone_level(0), 0);
    }

    #[test]
    fn preset_selectors_round_trip() {
        for p in EqPreset::STANDARD.iter().chain(EqPreset::ESPORTS.iter()) {
            assert_eq!(EqPreset::from_selector(p.selector()), Some(*p));
        }
        assert_eq!(EqPreset::from_selector(0x00), None);
    }
}
