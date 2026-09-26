//! Settings and profiles, stored as JSON in %APPDATA%\rzr\config.json
//! (~/.config/rzr/config.json elsewhere).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::mic::MicSettings;
use crate::protocol::{self, EqPreset, EQ_BANDS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqMode {
    Standard,
    Esports,
}

/// Everything the headset itself stores. Applied on connect and on change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub eq_mode: EqMode,
    /// Selected preset within each mode, like Synapse remembers them.
    pub standard_preset: EqPreset,
    pub esports_preset: EqPreset,
    pub custom_eq: [i8; EQ_BANDS],
    pub sidetone_enabled: bool,
    /// 1-100, Synapse's scale (see protocol::sidetone_level for the wire value).
    pub sidetone_volume: u8,
    pub dnd: bool,
    pub auto_off_enabled: bool,
    /// 15-60 minutes.
    pub auto_off_minutes: u8,
    /// Microphone enhancements (THX, plus the EQ preset index on the headset).
    pub mic: MicSettings,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Predeterminado".to_string(),
            eq_mode: EqMode::Standard,
            standard_preset: EqPreset::Custom,
            esports_preset: EqPreset::ApexLegends,
            custom_eq: [1, -2, 1, -3, 1, -3, -5, 2, 2, 3],
            sidetone_enabled: false,
            sidetone_volume: 50,
            dnd: false,
            auto_off_enabled: false,
            auto_off_minutes: 15,
            mic: MicSettings::default(),
        }
    }
}

impl Profile {
    /// The preset the headset should be playing.
    pub fn active_preset(&self) -> EqPreset {
        match self.eq_mode {
            EqMode::Standard => self.standard_preset,
            EqMode::Esports => self.esports_preset,
        }
    }

    /// Select a preset, switching mode to the preset's family.
    pub fn select_preset(&mut self, preset: EqPreset) {
        if preset.is_esports() {
            self.eq_mode = EqMode::Esports;
            self.esports_preset = preset;
        } else {
            self.eq_mode = EqMode::Standard;
            self.standard_preset = preset;
        }
    }

    /// Curve currently in effect (custom bands, or the preset's reference curve).
    pub fn active_curve(&self) -> [i8; EQ_BANDS] {
        self.active_preset().curve().unwrap_or(self.custom_eq)
    }

    /// Sidetone level to send: 0 = off.
    pub fn sidetone_wire(&self) -> u8 {
        if self.sidetone_enabled {
            protocol::sidetone_level(self.sidetone_volume).max(1)
        } else {
            0
        }
    }

    /// Auto power-off minutes to send: 0 = off.
    pub fn auto_off_wire(&self) -> u8 {
        if self.auto_off_enabled {
            self.auto_off_minutes
        } else {
            0
        }
    }

    /// Clamp every field into the range the headset accepts.
    pub fn sanitize(&mut self) {
        for b in self.custom_eq.iter_mut() {
            *b = (*b).clamp(protocol::EQ_MIN_DB, protocol::EQ_MAX_DB);
        }
        if self.standard_preset.is_esports() {
            self.standard_preset = EqPreset::Custom;
        }
        if !self.esports_preset.is_esports() {
            self.esports_preset = EqPreset::ApexLegends;
        }
        self.sidetone_volume = self.sidetone_volume.clamp(1, 100);
        self.auto_off_minutes =
            self.auto_off_minutes.clamp(protocol::AUTO_OFF_MIN_MINUTES, protocol::AUTO_OFF_MAX_MINUTES);
        if self.name.trim().is_empty() {
            self.name = "Perfil".to_string();
        }
        self.mic.sanitize();
    }
}

/// How EQ changes are sent. The guided test in AJUSTES picks whichever one
/// the user can actually hear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqMethod {
    /// Synapse/OpenRazer sequences with read-back verification.
    Verified,
    /// The first rzr release's sequence, sent twice, no read-back.
    Original,
    /// Verified write, then switch to another preset and back, so the
    /// headset loads the freshly written curve (guided test, round 2).
    Relatch,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub profiles: Vec<Profile>,
    pub active: usize,
    /// Windows endpoint IDs to make default on connect; empty = don't change.
    pub default_speaker: String,
    pub default_microphone: String,
    /// How long `rzr apply` waits for the dongle.
    pub wait_timeout_ms: u32,
    /// Send Synapse's startup frames (dongle query, 0x9E = 0) before a full
    /// apply. Field name kept from earlier releases.
    pub send_legacy_config: bool,
    /// Value sent for "Speaker Preset EQ Status" (0x9E) in that sequence.
    pub eq_status: u8,
    pub eq_method: EqMethod,
    /// Hand control back to the headset (remote mode off) after each
    /// command, as OpenRazer does. The first rzr release never did.
    pub release_remote: bool,
    /// Write debug.log (every HID frame and what the app was doing).
    pub debug_log: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            profiles: vec![Profile::default()],
            active: 0,
            default_speaker: String::new(),
            default_microphone: String::new(),
            wait_timeout_ms: 5000,
            send_legacy_config: true,
            eq_status: 0,
            eq_method: EqMethod::Verified,
            release_remote: true,
            debug_log: false,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("rzr").join("config.json")
    }

    /// Load the config. On first run, migrate the pre-GUI registry settings.
    pub fn load() -> Self {
        let mut cfg = match std::fs::read_to_string(Self::path()) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => crate::registry::legacy_config().unwrap_or_default(),
        };
        cfg.sanitize();
        cfg
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("No se pudo guardar: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("No se pudo guardar: {e}"))
    }

    pub fn sanitize(&mut self) {
        if self.profiles.is_empty() {
            self.profiles.push(Profile::default());
        }
        for p in self.profiles.iter_mut() {
            p.sanitize();
        }
        if self.active >= self.profiles.len() {
            self.active = 0;
        }
    }

    pub fn profile(&self) -> &Profile {
        &self.profiles[self.active]
    }

    pub fn profile_mut(&mut self) -> &mut Profile {
        &mut self.profiles[self.active]
    }

    /// A name not used by any profile yet, based on `base`.
    pub fn unique_name(&self, base: &str) -> String {
        if !self.profiles.iter().any(|p| p.name == base) {
            return base.to_string();
        }
        (2..).map(|n| format!("{base} ({n})")).find(|name| !self.profiles.iter().any(|p| &p.name == name)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_clamps_ranges() {
        let mut p = Profile {
            custom_eq: [9, -9, 0, 0, 0, 0, 0, 0, 0, 0],
            sidetone_volume: 0,
            auto_off_minutes: 90,
            ..Profile::default()
        };
        p.sanitize();
        assert_eq!(p.custom_eq[0], 5);
        assert_eq!(p.custom_eq[1], -5);
        assert_eq!(p.sidetone_volume, 1);
        assert_eq!(p.auto_off_minutes, 60);
    }

    #[test]
    fn select_preset_switches_mode() {
        let mut p = Profile::default();
        p.select_preset(EqPreset::Valorant);
        assert_eq!(p.eq_mode, EqMode::Esports);
        assert_eq!(p.active_preset(), EqPreset::Valorant);
        p.select_preset(EqPreset::Music);
        assert_eq!(p.active_preset(), EqPreset::Music);
        assert_eq!(p.esports_preset, EqPreset::Valorant);
    }

    #[test]
    fn partial_json_gets_defaults() {
        let cfg: Config = serde_json::from_str(r#"{"profiles":[{"name":"A","dnd":true}]}"#).unwrap();
        assert!(cfg.profiles[0].dnd);
        assert_eq!(cfg.profiles[0].sidetone_volume, 50);
        assert_eq!(cfg.wait_timeout_ms, 5000);
    }
}
