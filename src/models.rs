//! Headset models rzr knows about. Only fully supported models are ever sent
//! anything; for the rest, the diagnostics (src/diagnostics.rs) only read
//! what the device reports, to learn enough to support it later.

use serde::{Deserialize, Serialize};

/// Razer's USB vendor ID.
pub const RAZER_VID: u16 = 0x1532;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeadsetModel {
    /// The 2.4 GHz + Bluetooth revision with USB-C charging (dongle 1532:0555).
    #[default]
    #[serde(rename = "blackshark_v2_pro_2023")]
    BlackSharkV2Pro2023,
    /// The original wireless model with micro-USB charging (dongle 1532:0528).
    #[serde(rename = "blackshark_v2_pro_2020")]
    BlackSharkV2Pro2020,
    /// Any other Razer headset: diagnostics look at every Razer device.
    #[serde(rename = "other")]
    Other,
}

impl HeadsetModel {
    pub const ALL: [HeadsetModel; 3] = [Self::BlackSharkV2Pro2023, Self::BlackSharkV2Pro2020, Self::Other];

    pub fn name(self) -> &'static str {
        match self {
            Self::BlackSharkV2Pro2023 => "Razer BlackShark V2 Pro (2023)",
            Self::BlackSharkV2Pro2020 => "Razer BlackShark V2 Pro (2020)",
            Self::Other => "Other Razer headset",
        }
    }

    /// How to tell it apart, for the model picker.
    pub fn hint(self) -> &'static str {
        match self {
            Self::BlackSharkV2Pro2023 => "2.4 GHz + Bluetooth, charges over USB-C · dongle 1532:0555",
            Self::BlackSharkV2Pro2020 => "2.4 GHz only, charges over micro-USB · dongle 1532:0528",
            Self::Other => "Not listed here · diagnostics look at every Razer device",
        }
    }

    /// USB product ID of its dongle; None for "other".
    pub fn pid(self) -> Option<u16> {
        match self {
            Self::BlackSharkV2Pro2023 => Some(0x0555),
            Self::BlackSharkV2Pro2020 => Some(0x0528),
            Self::Other => None,
        }
    }

    /// Whether rzr controls it. Unsupported models are never written to.
    pub fn supported(self) -> bool {
        self == Self::BlackSharkV2Pro2023
    }

    /// Whether a Razer device with this product ID belongs to the model.
    pub fn matches(self, pid: u16) -> bool {
        self.pid().is_none_or(|p| p == pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_2023_model_is_supported() {
        let supported: Vec<_> = HeadsetModel::ALL.into_iter().filter(|m| m.supported()).collect();
        assert_eq!(supported, [HeadsetModel::BlackSharkV2Pro2023]);
    }

    #[test]
    fn other_matches_every_razer_device() {
        assert!(HeadsetModel::Other.matches(0x0528));
        assert!(HeadsetModel::BlackSharkV2Pro2020.matches(0x0528));
        assert!(!HeadsetModel::BlackSharkV2Pro2020.matches(0x0555));
    }

    #[test]
    fn ids_for_the_page_and_config() {
        assert_eq!(serde_json::to_string(&HeadsetModel::BlackSharkV2Pro2020).unwrap(), "\"blackshark_v2_pro_2020\"");
        assert_eq!(serde_json::from_str::<HeadsetModel>("\"other\"").unwrap(), HeadsetModel::Other);
    }
}
