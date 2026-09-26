//! Read-only diagnostics for a headset model, saved as a text report the user
//! can send (Settings › Diagnostics, or `rzr diagnose`).
//!
//! Nothing is ever sent to a model rzr doesn't support: for those it only
//! lists the Razer USB devices with their HID report descriptors, and records
//! the input reports the device sends on its own while the user presses its
//! buttons. For the supported model it also runs the same read-only queries
//! the panel uses. Serial numbers and device paths are left out of the report.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use hidapi::{DeviceInfo, HidApi, HidDevice};

use crate::config::Config;
use crate::models::{HeadsetModel, RAZER_VID};
use crate::winaudio::{self, Flow};

/// How long to record input reports by default.
pub const LISTEN: Duration = Duration::from_secs(20);
/// Stop recording input reports past this many (a chatty device).
const MAX_REPORTS: usize = 2000;

/// Where a run is, for the page.
#[derive(Clone, Debug, PartialEq)]
pub enum Progress {
    Step(&'static str),
    /// Recording input reports: seconds left and reports seen so far.
    Listening {
        left: u32,
        reports: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub path: PathBuf,
    /// A few lines for the page: what was found.
    pub summary: Vec<String>,
}

/// Folder the reports are saved in.
pub fn dir() -> PathBuf {
    Config::path().with_file_name("diagnostics")
}

/// Run the diagnostics for `model`, recording input reports for `listen`.
pub fn run(model: HeadsetModel, listen: Duration, progress: &dyn Fn(Progress)) -> Result<Report, String> {
    winaudio::com_init();
    let mut out = String::new();
    let mut summary = Vec::new();
    let now = chrono::Local::now();

    let _ = writeln!(out, "# rzr diagnostics report\n");
    let _ = writeln!(out, "- rzr {} | {} | {}", env!("CARGO_PKG_VERSION"), os_version(), std::env::consts::ARCH);
    let _ = writeln!(out, "- Model picked: {} ({})", model.name(), pid_text(model));
    let _ = writeln!(out, "- Date: {}", now.format("%Y-%m-%d %H:%M"));

    progress(Progress::Step("Looking for Razer devices…"));
    let api = HidApi::new().map_err(|e| format!("Could not start HID: {e}"))?;
    let razer: Vec<&DeviceInfo> = api.device_list().filter(|i| i.vendor_id() == RAZER_VID).collect();
    let _ = writeln!(out, "\n## Razer USB devices (HID)\n");
    if razer.is_empty() {
        let _ = writeln!(out, "None found. Is the dongle (or the headset's cable) plugged in?");
    }
    for info in &razer {
        describe(&mut out, &api, info);
    }
    let mine: Vec<&DeviceInfo> = razer.iter().copied().filter(|i| model.matches(i.product_id())).collect();
    let mut pids: Vec<u16> = mine.iter().map(|i| i.product_id()).collect();
    pids.dedup();
    summary.push(match (mine.is_empty(), model.pid()) {
        (true, Some(pid)) => format!("{} (1532:{pid:04X}) was not found.", model.name()),
        (true, None) => "No Razer device was found.".to_string(),
        (false, _) => format!(
            "Found {} ({} HID collection{}).",
            pids.iter().map(|p| format!("1532:{p:04X}")).collect::<Vec<_>>().join(", "),
            mine.len(),
            if mine.len() == 1 { "" } else { "s" }
        ),
    });

    if model.supported() && !mine.is_empty() {
        progress(Progress::Step("Reading the headset…"));
        let _ = writeln!(out, "\n## Headset readings\n");
        read_supported(&mut out);
    }

    let listened = listen_to(&api, &mine, listen, &mut out, progress);
    if !mine.is_empty() {
        summary
            .push(format!("Recorded {listened} input report{} while listening.", if listened == 1 { "" } else { "s" }));
    }

    progress(Progress::Step("Checking Windows audio and THX…"));
    let _ = writeln!(out, "\n## Windows audio\n");
    let thx = describe_audio(&mut out);
    summary.push(if thx { "THX is on the headset's output." } else { "THX was not found on a Razer output." }.into());

    progress(Progress::Step("Saving the report…"));
    let dir = dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    let id = serde_json::to_value(model).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    let path = dir.join(format!("rzr-diagnostics-{id}-{}.txt", now.format("%Y%m%d-%H%M%S")));
    std::fs::write(&path, out).map_err(|e| format!("Could not save the report: {e}"))?;
    crate::dlog!("diagnostics saved: {}", path.display());
    Ok(Report { path, summary })
}

fn pid_text(model: HeadsetModel) -> String {
    model.pid().map_or_else(|| "any 1532:xxxx".to_string(), |p| format!("1532:{p:04X}"))
}

/// One HID collection: IDs, strings and its report descriptor.
fn describe(out: &mut String, api: &HidApi, info: &DeviceInfo) {
    let _ = writeln!(
        out,
        "- 1532:{:04X} | interface {} | usage page {:04X} usage {:04X} | \"{}\" by \"{}\" | release {:04X}",
        info.product_id(),
        info.interface_number(),
        info.usage_page(),
        info.usage(),
        info.product_string().unwrap_or(""),
        info.manufacturer_string().unwrap_or(""),
        info.release_number(),
    );
    match info.open_device(api) {
        Ok(dev) => {
            let mut buf = [0u8; 4096];
            match dev.get_report_descriptor(&mut buf) {
                Ok(n) => {
                    let _ = writeln!(out, "  - Report descriptor ({n} bytes): {}", hex(&buf[..n]));
                }
                Err(e) => {
                    let _ = writeln!(out, "  - Report descriptor: not readable ({e})");
                }
            }
        }
        // Windows keeps keyboards and mice to itself; that's expected.
        Err(e) => {
            let _ = writeln!(out, "  - Could not open it ({e})");
        }
    }
}

/// The panel's own read-only queries, for the supported model.
fn read_supported(out: &mut String) {
    let dev = match crate::device::Device::open(1500) {
        Ok(d) => d,
        Err(e) => {
            let _ = writeln!(out, "Could not open the dongle: {e}");
            return;
        }
    };
    let st = match dev.status() {
        Ok(st) => st,
        Err(e) => {
            let _ = writeln!(out, "Could not read the headset: {e}");
            return;
        }
    };
    let opt = |v: Option<String>| v.unwrap_or_else(|| "no reply".into());
    let _ = writeln!(out, "- Headset linked: {}", if st.headset_connected { "yes" } else { "no" });
    let _ = writeln!(out, "- Dongle firmware: {}", opt(dev.get_dongle_firmware()));
    if st.headset_connected {
        let _ = writeln!(out, "- Battery: {}", opt(st.battery.map(|b| format!("{b}%"))));
        let _ = writeln!(out, "- Charging: {}", opt(st.charging.map(|c| c.to_string())));
        let _ = writeln!(out, "- Mic muted (button): {}", opt(st.mic_muted.map(|m| m.to_string())));
        let _ = writeln!(out, "- Headset firmware: {}", opt(dev.get_firmware()));
        let _ = writeln!(
            out,
            "- Serial number: {}",
            if dev.get_serial().is_some() { "present (not included)" } else { "no reply" }
        );
        let _ = writeln!(out, "- EQ: {}", dev.eq_readback());
    }
}

/// Record the input reports the model's collections send on their own.
/// Returns how many were seen.
fn listen_to(
    api: &HidApi,
    infos: &[&DeviceInfo],
    listen: Duration,
    out: &mut String,
    progress: &dyn Fn(Progress),
) -> usize {
    let _ = writeln!(out, "\n## Input reports while listening ({} s)\n", listen.as_secs());
    let handles: Vec<(String, HidDevice)> = infos
        .iter()
        .filter_map(|i| {
            let dev = i.open_device(api).ok()?;
            let name = format!(
                "1532:{:04X} if{} {:04X}:{:04X}",
                i.product_id(),
                i.interface_number(),
                i.usage_page(),
                i.usage()
            );
            Some((name, dev))
        })
        .collect();
    if handles.is_empty() {
        let _ = writeln!(out, "Nothing to listen to.");
        return 0;
    }
    let _ = writeln!(out, "Listening to: {}\n", handles.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "));

    let start = Instant::now();
    let end = start + listen;
    let mut seen = 0;
    let mut buf = [0u8; 512];
    let mut last_left = u32::MAX;
    while Instant::now() < end && seen < MAX_REPORTS {
        let left = end.saturating_duration_since(Instant::now()).as_secs_f32().ceil() as u32;
        if left != last_left {
            last_left = left;
            progress(Progress::Listening { left, reports: seen });
        }
        let mut quiet = true;
        for (name, dev) in &handles {
            while let Ok(n) = dev.read_timeout(&mut buf, 0) {
                if n == 0 || seen >= MAX_REPORTS {
                    break;
                }
                quiet = false;
                seen += 1;
                let at = start.elapsed().as_secs_f32();
                let _ = writeln!(out, "- +{at:6.3} s | {name} | {}", hex(&buf[..n]));
            }
        }
        if quiet {
            thread::sleep(Duration::from_millis(10));
        }
    }
    if seen == 0 {
        let _ = writeln!(out, "No input reports.");
    }
    seen
}

/// Razer audio endpoints and whether THX is on them. Returns true if THX is.
fn describe_audio(out: &mut String) -> bool {
    let mut thx = false;
    for (flow, label) in [(Flow::Render, "Output"), (Flow::Capture, "Input")] {
        let razer: Vec<_> = winaudio::list_devices(flow)
            .into_iter()
            .filter(|d| {
                let n = d.name.to_lowercase();
                n.contains("razer") || n.contains("blackshark")
            })
            .collect();
        if razer.is_empty() {
            let _ = writeln!(out, "- {label}: no Razer device");
        }
        for d in razer {
            let default = if d.is_default { " (default)" } else { "" };
            let _ = writeln!(out, "- {label}: \"{}\"{default}", d.name);
            if flow == Flow::Render {
                match crate::thx::read_state(&d.id) {
                    Some(Ok(s)) => {
                        thx = true;
                        let _ = writeln!(
                            out,
                            "  - THX: preset \"{}\", spatial {}, bass boost {}",
                            s.preset_name, s.spatial_enabled, s.bass_boost_enabled
                        );
                    }
                    Some(Err(e)) => {
                        let _ = writeln!(out, "  - THX: unreadable state ({e})");
                    }
                    None => {
                        let _ = writeln!(out, "  - THX: not installed on this output");
                    }
                }
            }
        }
    }
    let service = crate::thx::Service::connect().is_ok();
    let _ = writeln!(out, "- THX service: {}", if service { "answers" } else { "does not answer" });
    thx
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

#[cfg(windows)]
fn os_version() -> String {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let get = |name: &str| key.as_ref().ok().and_then(|k| k.get_value::<String, _>(name).ok()).unwrap_or_default();
    let ubr = key.as_ref().ok().and_then(|k| k.get_value::<u32, _>("UBR").ok()).unwrap_or(0);
    // ProductName still says "Windows 10" on Windows 11; the build tells them apart.
    format!("Windows build {}.{ubr} ({})", get("CurrentBuild"), get("DisplayVersion"))
}

#[cfg(not(windows))]
fn os_version() -> String {
    std::env::consts::OS.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_keeps_every_byte() {
        assert_eq!(hex(&[0x06, 0x00, 0xFF, 0x00]), "06 00 FF 00");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn reports_have_their_own_folder() {
        // Not compared with Config::path(): another test changes APPDATA.
        assert_eq!(dir().file_name().unwrap(), "diagnostics");
    }
}
