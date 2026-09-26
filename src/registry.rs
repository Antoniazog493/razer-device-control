/// Windows registry: migration of the pre-GUI settings (HKCU\SOFTWARE\rzr)
/// and the "start with Windows" entry (HKCU\...\CurrentVersion\Run).

use crate::config::Config;

/// Build a Config from the settings older rzr versions kept in the registry.
#[cfg(windows)]
pub fn legacy_config() -> Option<Config> {
    use winreg::enums::*;
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("SOFTWARE\\rzr")
        .ok()?;
    let mut cfg = Config::default();

    if let Ok(v) = key.get_value::<String, _>("eq_bands") {
        if let Some(bands) = parse_eq_bands(&v) {
            cfg.profiles[0].custom_eq = bands;
        }
    }
    if let Ok(v) = key.get_value::<u32, _>("wait_timeout_ms") {
        cfg.wait_timeout_ms = v;
    }
    if let Ok(v) = key.get_value::<String, _>("default_speaker") {
        cfg.default_speaker = v;
    }
    if let Ok(v) = key.get_value::<String, _>("default_microphone") {
        cfg.default_microphone = v;
    }
    // "volume" was really the preset selector (255 = custom) and "enhancement"
    // the preset-family flag, so neither carries over.
    Some(cfg)
}

#[cfg(not(windows))]
pub fn legacy_config() -> Option<Config> {
    None
}

#[cfg_attr(not(windows), allow(dead_code))]
fn parse_eq_bands(s: &str) -> Option<[i8; 10]> {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.len() != 10 {
        return None;
    }
    let mut bands = [0i8; 10];
    for (i, part) in parts.iter().enumerate() {
        bands[i] = part.parse::<i8>().ok()?;
    }
    Some(bands)
}

#[cfg(windows)]
const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "rzr";

/// Whether rzr is registered to start (in background watch mode) at login.
#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(RUN_VALUE))
        .is_ok()
}

#[cfg(windows)]
pub fn set_autostart(enable: bool) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(RUN_KEY)
        .map_err(|e| format!("No se pudo abrir el registro: {e}"))?;
    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let cmd = format!("\"{}\" --silent --watch", exe.display());
        key.set_value(RUN_VALUE, &cmd)
            .map_err(|e| format!("No se pudo escribir en el registro: {e}"))
    } else {
        match key.delete_value(RUN_VALUE) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("No se pudo escribir en el registro: {e}")),
        }
    }
}

#[cfg(not(windows))]
pub fn autostart_enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_autostart(_enable: bool) -> Result<(), String> {
    Err("Solo disponible en Windows".to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_legacy_bands() {
        assert_eq!(
            super::parse_eq_bands("1,-2,1,-3,1,-3,-5,2,2,3"),
            Some([1, -2, 1, -3, 1, -3, -5, 2, 2, 3])
        );
        assert_eq!(super::parse_eq_bands("1,2"), None);
    }
}
