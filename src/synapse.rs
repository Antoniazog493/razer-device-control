/// Import of Razer Synapse 4 profile exports (*.synapse4).
///
/// The file is JSON: { deviceName, profiles: [{ name, payload }] } where each
/// payload is base64-encoded JSON with the profile's settings.

use base64::Engine;
use serde_json::Value;

use crate::config::{EqMode, Profile};
use crate::protocol::{self, EqPreset, EQ_BANDS};

pub fn import_file(path: &std::path::Path) -> Result<Vec<Profile>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("No se pudo leer el archivo: {e}"))?;
    import_str(&text)
}

pub fn import_str(text: &str) -> Result<Vec<Profile>, String> {
    let root: Value = serde_json::from_str(text).map_err(|_| "No es un archivo .synapse4 válido".to_string())?;
    let entries = root["profiles"]
        .as_array()
        .ok_or("El archivo no contiene perfiles")?;

    let mut out = Vec::new();
    for entry in entries {
        let Some(b64) = entry["payload"].as_str() else { continue };
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) else { continue };
        let Ok(payload) = serde_json::from_slice::<Value>(&bytes) else { continue };
        let name = payload["name"]
            .as_str()
            .or_else(|| entry["name"].as_str())
            .unwrap_or("Synapse");
        out.push(profile_from_payload(name, &payload));
    }

    if out.is_empty() {
        return Err("No se encontró ningún perfil válido en el archivo".to_string());
    }
    Ok(out)
}

fn preset_by_name(name: &str) -> Option<EqPreset> {
    Some(match name {
        "game" => EqPreset::Game,
        "movie" => EqPreset::Movie,
        "music" => EqPreset::Music,
        "custom" => EqPreset::Custom,
        "apexLegends" => EqPreset::ApexLegends,
        "callOfDuty" => EqPreset::CallOfDuty,
        "csgo" => EqPreset::Csgo,
        "fortnite" => EqPreset::Fortnite,
        "valorant" => EqPreset::Valorant,
        _ => return None,
    })
}

fn bands(v: &Value) -> Option<[i8; EQ_BANDS]> {
    let arr = v.as_array()?;
    if arr.len() != EQ_BANDS {
        return None;
    }
    let mut out = [0i8; EQ_BANDS];
    for (o, x) in out.iter_mut().zip(arr) {
        *o = x.as_f64()?.round().clamp(-128.0, 127.0) as i8;
    }
    Some(out)
}

fn profile_from_payload(name: &str, p: &Value) -> Profile {
    let mut prof = Profile {
        name: name.to_string(),
        ..Profile::default()
    };

    let eq = &p["audioEqualizer"];
    if eq["selectedMode"].as_str() == Some("esports") {
        prof.eq_mode = EqMode::Esports;
    }
    if let Some(preset) = eq["selectedPreset"]["standard"].as_str().and_then(preset_by_name) {
        prof.standard_preset = preset;
    }
    if let Some(preset) = eq["selectedPreset"]["esports"].as_str().and_then(preset_by_name) {
        prof.esports_preset = preset;
    }
    let custom = eq["presets"]["standard"]
        .as_array()
        .and_then(|list| list.iter().find(|x| x["name"] == "custom"))
        .and_then(|x| bands(&x["current"]));
    if let Some(custom) = custom {
        prof.custom_eq = custom;
    }

    let st = &p["micSideTone"];
    prof.sidetone_enabled = st["isEnabled"].as_bool().unwrap_or(false);
    if let Some(v) = st["value"].as_f64() {
        // Synapse shows 0-100, the headset takes 0-10.
        prof.sidetone_level = (v / 10.0).round().clamp(1.0, protocol::SIDETONE_MAX as f64) as u8;
    }

    prof.dnd = p["notDisturb"]["isEnabled"].as_bool().unwrap_or(false);

    let ps = &p["powerSaving"];
    prof.auto_off_enabled = ps["isEnabled"].as_bool().unwrap_or(false);
    if let Some(v) = ps["value"].as_f64() {
        prof.auto_off_minutes = v.round().clamp(0.0, 255.0) as u8;
    }

    prof.sanitize();
    prof
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export(payload: &str) -> String {
        let b64 = base64::engine::general_purpose::STANDARD.encode(payload);
        format!(r#"{{"productId":1365,"deviceName":"Razer BlackShark V2 Pro","category":"AUDIO","profiles":[{{"name":"EQP","payload":"{b64}"}}]}}"#)
    }

    #[test]
    fn imports_synapse_profile() {
        let payload = r#"{
            "name": "EQP",
            "micSideTone": {"isEnabled": true, "value": 60},
            "notDisturb": {"isEnabled": true},
            "powerSaving": {"isEnabled": true, "value": 30},
            "audioEqualizer": {
                "selectedMode": "standard",
                "presets": {"standard": [
                    {"name": "game", "default": [0,0,0,0,0,0,0,0,0,0], "current": [0,0,0,0,0,0,0,0,0,0]},
                    {"name": "custom", "default": [0,0,0,0,0,0,0,0,0,0], "current": [3,4,3,0,0,2,3,4,4,3]}
                ]},
                "selectedPreset": {"standard": "custom", "esports": "valorant"}
            }
        }"#;
        let profiles = import_str(&export(payload)).unwrap();
        assert_eq!(profiles.len(), 1);
        let p = &profiles[0];
        assert_eq!(p.name, "EQP");
        assert_eq!(p.active_preset(), EqPreset::Custom);
        assert_eq!(p.esports_preset, EqPreset::Valorant);
        assert_eq!(p.custom_eq, [3, 4, 3, 0, 0, 2, 3, 4, 4, 3]);
        assert!(p.sidetone_enabled);
        assert_eq!(p.sidetone_level, 6);
        assert!(p.dnd);
        assert!(p.auto_off_enabled);
        assert_eq!(p.auto_off_minutes, 30);
    }

    #[test]
    fn rejects_garbage() {
        assert!(import_str("not json").is_err());
        assert!(import_str(r#"{"profiles":[]}"#).is_err());
    }
}
