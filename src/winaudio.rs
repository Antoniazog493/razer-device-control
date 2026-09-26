/// Windows audio endpoints: listing, volume/mute and default device.
///
/// Volume and listing go through Core Audio (MMDevice API) directly. Setting
/// the default device uses the undocumented IPolicyConfig through PowerShell.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Speakers / headphones.
    Render,
    /// Microphones.
    Capture,
}

/// An audio endpoint device.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioDevice {
    pub name: String,
    pub id: String,
    pub is_default: bool,
}

/// Live volume state of one endpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    /// 0.0 - 1.0
    pub volume: f32,
    pub muted: bool,
}

/// Endpoint to control for the headset: the one named after it, else the default.
pub fn headset_endpoint(flow: Flow) -> Option<Endpoint> {
    let devices = list_devices(flow);
    let dev = devices
        .iter()
        .find(|d| d.name.to_lowercase().contains("blackshark"))
        .or_else(|| devices.iter().find(|d| d.is_default))?;
    let (volume, muted) = get_volume(&dev.id)?;
    Some(Endpoint {
        id: dev.id.clone(),
        name: dev.name.clone(),
        volume,
        muted,
    })
}

#[cfg(windows)]
pub use imp::*;

#[cfg(windows)]
mod imp {
    use super::{AudioDevice, Flow};
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use windows::core::{HSTRING, Result};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Initialise COM on the calling thread (once per thread; repeats are harmless).
    pub fn com_init() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn enumerator() -> Result<IMMDeviceEnumerator> {
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
    }

    fn device_id(dev: &IMMDevice) -> Option<String> {
        unsafe {
            let p = dev.GetId().ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as _));
            s
        }
    }

    fn device_name(dev: &IMMDevice) -> Option<String> {
        unsafe {
            let store = dev.OpenPropertyStore(STGM_READ).ok()?;
            let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
            Some(value.to_string())
        }
    }

    fn endpoint_volume(id: &str) -> Result<IAudioEndpointVolume> {
        unsafe {
            let dev = enumerator()?.GetDevice(&HSTRING::from(id))?;
            dev.Activate(CLSCTX_ALL, None)
        }
    }

    /// List active audio endpoints.
    pub fn list_devices(flow: Flow) -> Vec<AudioDevice> {
        let df = match flow {
            Flow::Render => eRender,
            Flow::Capture => eCapture,
        };
        let Ok(en) = enumerator() else { return Vec::new() };
        unsafe {
            let default_id = en
                .GetDefaultAudioEndpoint(df, eConsole)
                .ok()
                .and_then(|d| device_id(&d))
                .unwrap_or_default();
            let Ok(col) = en.EnumAudioEndpoints(df, DEVICE_STATE_ACTIVE) else { return Vec::new() };
            let count = col.GetCount().unwrap_or(0);
            (0..count)
                .filter_map(|i| {
                    let dev = col.Item(i).ok()?;
                    let id = device_id(&dev)?;
                    let name = device_name(&dev).unwrap_or_else(|| id.clone());
                    Some(AudioDevice {
                        is_default: id == default_id,
                        name,
                        id,
                    })
                })
                .collect()
        }
    }

    /// (volume 0.0-1.0, muted)
    pub fn get_volume(id: &str) -> Option<(f32, bool)> {
        let vol = endpoint_volume(id).ok()?;
        unsafe {
            let level = vol.GetMasterVolumeLevelScalar().ok()?;
            let muted = vol.GetMute().ok()?.as_bool();
            Some((level, muted))
        }
    }

    pub fn set_volume(id: &str, level: f32) -> bool {
        endpoint_volume(id)
            .and_then(|v| unsafe { v.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null()) })
            .is_ok()
    }

    pub fn set_mute(id: &str, muted: bool) -> bool {
        endpoint_volume(id)
            .and_then(|v| unsafe { v.SetMute(muted, std::ptr::null()) })
            .is_ok()
    }

    /// Set the default audio device (all roles) by endpoint ID.
    pub fn set_default_device(device_id: &str) -> bool {
        // Uses IPolicyConfig COM interface via PowerShell
        let ps_script = format!(r#"
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

[Guid("F8679F50-850A-41CF-9C72-430F290290C8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
internal interface IPolicyConfig {{
    int GetMixFormat(string id, IntPtr fmt);
    int GetDeviceFormat(string id, int def, IntPtr fmt);
    int ResetDeviceFormat(string id);
    int SetDeviceFormat(string id, IntPtr fmt, IntPtr fmtm);
    int GetProcessingPeriod(string id, int def, IntPtr defp, IntPtr minp);
    int SetProcessingPeriod(string id, IntPtr period);
    int GetShareMode(string id, IntPtr mode);
    int SetShareMode(string id, IntPtr mode);
    int GetPropertyValue(string id, int storeType, IntPtr key, IntPtr value);
    int SetPropertyValue(string id, int storeType, IntPtr key, IntPtr value);
    int SetDefaultEndpoint(string id, int role);
    int SetEndpointVisibility(string id, int visible);
}}

[ComImport, Guid("870AF99C-171D-4F9E-AF0D-E63DF40C2BC9")]
internal class PolicyConfigClient {{ }}

public class AudioSwitcher {{
    public static void SetDefault(string id) {{
        var policy = (IPolicyConfig)(new PolicyConfigClient());
        policy.SetDefaultEndpoint(id, 0); // eConsole
        policy.SetDefaultEndpoint(id, 1); // eMultimedia
        policy.SetDefaultEndpoint(id, 2); // eCommunications
    }}
}}
'@
[AudioSwitcher]::SetDefault("{id}")
"#, id = device_id.replace('"', ""));

        Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn launch(program: &str, args: &[&str]) {
        let _ = Command::new(program)
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }

    pub fn open_volume_mixer() {
        launch("sndvol.exe", &[]);
    }

    pub fn open_sound_settings() {
        launch("control.exe", &["mmsys.cpl"]);
    }

    /// Open a file or folder with its default app.
    pub fn open_path(path: &std::path::Path) {
        let _ = Command::new("explorer.exe").arg(path).spawn();
    }
}

/// Non-Windows builds (development only): no system audio integration.
#[cfg(not(windows))]
mod imp {
    use super::{AudioDevice, Flow};

    pub fn com_init() {}
    pub fn list_devices(_flow: Flow) -> Vec<AudioDevice> {
        Vec::new()
    }
    pub fn get_volume(_id: &str) -> Option<(f32, bool)> {
        None
    }
    pub fn set_volume(_id: &str, _level: f32) -> bool {
        false
    }
    pub fn set_mute(_id: &str, _muted: bool) -> bool {
        false
    }
    pub fn set_default_device(_id: &str) -> bool {
        false
    }
    pub fn open_volume_mixer() {}
    pub fn open_sound_settings() {}
    pub fn open_path(path: &std::path::Path) {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

#[cfg(not(windows))]
pub use imp::*;
