//! Microphone enhancements: EQ, normalization, voice clarity, noise
//! reduction and voice gate. THX's microphone effect does them, not the
//! headset; its service keeps them only while it runs, so they live in the
//! profile and rzr sends them again (ADR 0007). The headset only gets the
//! EQ preset's index (0x96), as Synapse sends it.

use serde::{Deserialize, Serialize};

use crate::protocol::EQ_BANDS;

/// Range of the microphone EQ in Synapse, in dB.
pub const MIC_EQ_MIN_DB: i8 = -12;
pub const MIC_EQ_MAX_DB: i8 = 12;
/// Voice gate threshold range, in dB (what Synapse sends).
pub const GATE_MIN_DB: i16 = -40;
pub const GATE_MAX_DB: i16 = -20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicEqPreset {
    #[default]
    Default,
    MicBoost,
    Broadcast,
    Conference,
    Custom,
}

impl MicEqPreset {
    pub const ALL: [MicEqPreset; 5] = [Self::Default, Self::MicBoost, Self::Broadcast, Self::Conference, Self::Custom];

    /// Value of "Set Mic Preset EQ Index" (0x96), from Synapse's capture.
    pub fn selector(self) -> u8 {
        match self {
            Self::Default => 0,
            Self::MicBoost => 1,
            Self::Broadcast => 2,
            Self::Conference => 3,
            Self::Custom => 255,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::MicBoost => "Mic Boost",
            Self::Broadcast => "Broadcast",
            Self::Conference => "Conference",
            Self::Custom => "Custom",
        }
    }

    /// The curve Synapse sends to THX for each preset (its log, 2026-09-26).
    pub fn curve(self) -> Option<[i8; EQ_BANDS]> {
        Some(match self {
            Self::Default => [0; EQ_BANDS],
            Self::MicBoost => [0, 2, 3, 4, 5, 5, 5, 4, 3, 1],
            Self::Broadcast => [4, 4, 4, 3, -2, -7, -4, -2, -3, -5],
            Self::Conference => [-8, -7, -5, -3, -1, 1, 3, 2, 1, 0],
            Self::Custom => return None,
        })
    }
}

/// An enhancement with a switch and a level (0-100, or dB for the gate).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    pub on: bool,
    pub level: i16,
}

/// Which enhancement, for commands from the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MicEffect {
    Normalization,
    VoiceClarity,
    NoiseReduction,
    VoiceGate,
}

impl MicEffect {
    /// The (switch, level) parameters of `IVSSrvSettings::SetMicParams`
    /// (found by watching them while Synapse changed each one).
    pub fn params(self) -> (u32, u32) {
        match self {
            MicEffect::Normalization => (14, 15),
            MicEffect::VoiceClarity => (10, 11),
            MicEffect::NoiseReduction => (6, 7),
            MicEffect::VoiceGate => (2, 3),
        }
    }

    /// The name the page uses (same as in its commands).
    pub fn id(self) -> &'static str {
        match self {
            MicEffect::Normalization => "normalization",
            MicEffect::VoiceClarity => "voice_clarity",
            MicEffect::NoiseReduction => "noise_reduction",
            MicEffect::VoiceGate => "voice_gate",
        }
    }

    pub fn range(self) -> (i16, i16) {
        match self {
            MicEffect::VoiceGate => (GATE_MIN_DB, GATE_MAX_DB),
            _ => (0, 100),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MicSettings {
    pub eq_preset: MicEqPreset,
    /// Custom, −12 to +12 dB.
    pub custom_eq: [i8; EQ_BANDS],
    pub normalization: Effect,
    pub voice_clarity: Effect,
    pub noise_reduction: Effect,
    /// Level = threshold in dB.
    pub voice_gate: Effect,
}

impl Default for MicSettings {
    /// Synapse's defaults: everything off, levels at 55; the gate in the
    /// middle of its range.
    fn default() -> Self {
        let off = |level| Effect { on: false, level };
        Self {
            eq_preset: MicEqPreset::Default,
            custom_eq: [0; EQ_BANDS],
            normalization: off(55),
            voice_clarity: off(55),
            noise_reduction: off(55),
            voice_gate: off(-30),
        }
    }
}

impl MicSettings {
    pub fn effect(&self, e: MicEffect) -> Effect {
        match e {
            MicEffect::Normalization => self.normalization,
            MicEffect::VoiceClarity => self.voice_clarity,
            MicEffect::NoiseReduction => self.noise_reduction,
            MicEffect::VoiceGate => self.voice_gate,
        }
    }

    pub fn effect_mut(&mut self, e: MicEffect) -> &mut Effect {
        match e {
            MicEffect::Normalization => &mut self.normalization,
            MicEffect::VoiceClarity => &mut self.voice_clarity,
            MicEffect::NoiseReduction => &mut self.noise_reduction,
            MicEffect::VoiceGate => &mut self.voice_gate,
        }
    }

    /// The curve in effect: the preset's, or the custom one.
    pub fn curve(&self) -> [i8; EQ_BANDS] {
        self.eq_preset.curve().unwrap_or(self.custom_eq)
    }

    pub const EFFECTS: [MicEffect; 4] =
        [MicEffect::Normalization, MicEffect::VoiceClarity, MicEffect::NoiseReduction, MicEffect::VoiceGate];

    pub fn sanitize(&mut self) {
        for b in self.custom_eq.iter_mut() {
            *b = (*b).clamp(MIC_EQ_MIN_DB, MIC_EQ_MAX_DB);
        }
        for e in Self::EFFECTS {
            let (min, max) = e.range();
            let fx = self.effect_mut(e);
            fx.level = fx.level.clamp(min, max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_match_synapse() {
        let sel: Vec<u8> = MicEqPreset::ALL.iter().map(|p| p.selector()).collect();
        assert_eq!(sel, [0, 1, 2, 3, 255]);
    }

    #[test]
    fn curve_follows_the_preset() {
        let mut m = MicSettings { custom_eq: [12, 0, 0, 0, 0, 12, 0, 0, 0, -12], ..Default::default() };
        assert_eq!(m.curve(), [0; EQ_BANDS]);
        m.eq_preset = MicEqPreset::Conference;
        assert_eq!(m.curve(), [-8, -7, -5, -3, -1, 1, 3, 2, 1, 0]);
        m.eq_preset = MicEqPreset::Custom;
        assert_eq!(m.curve(), [12, 0, 0, 0, 0, 12, 0, 0, 0, -12]);
    }

    #[test]
    fn sanitize_clamps_levels_and_bands() {
        let mut m = MicSettings {
            custom_eq: [20, -20, 0, 0, 0, 0, 0, 0, 0, 0],
            normalization: Effect { on: true, level: 300 },
            voice_gate: Effect { on: true, level: 0 },
            ..Default::default()
        };
        m.sanitize();
        assert_eq!(&m.custom_eq[..2], &[12, -12]);
        assert_eq!(m.normalization.level, 100);
        assert_eq!(m.voice_gate.level, -20);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let m: MicSettings = serde_json::from_str(r#"{"eq_preset":"broadcast"}"#).unwrap();
        assert_eq!(m.eq_preset, MicEqPreset::Broadcast);
        assert_eq!(m.voice_gate, Effect { on: false, level: -30 });
    }
}
