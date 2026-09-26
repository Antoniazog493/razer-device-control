// GUI-subsystem on Windows: no console window flashes on double-click or at
// login. CLI commands attach to the parent console instead.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod connlog;
mod debuglog;
mod device;
mod diagnostics;
mod gui;
mod instance;
mod mic;
mod models;
mod protocol;
mod registry;
mod synapse;
mod thx;
mod winaudio;
mod worker;

use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

use config::Config;
use mic::MicSettings;
use thx::ThxEq;

#[cfg(windows)]
extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
}

/// Reuse the console of the shell that launched us, so CLI output shows up.
#[cfg(windows)]
fn attach_console() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_console() {}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |flags: &[&str]| args.iter().any(|a| flags.contains(&a.as_str()));
    let silent = has(&["--silent", "-s"]);
    let watch = has(&["--watch", "-w"]);
    let debug = has(&["--debug"]);

    let source = if watch {
        "background"
    } else if silent || args.len() > 1 && !has(&["--demo", "--debug"]) {
        "cli"
    } else {
        "panel"
    };
    debuglog::init(source, debug);
    debuglog::set_enabled(Config::load().debug_log);
    dlog!("start: {:?}", &args[1..]);

    if silent {
        if watch {
            run_watch(true);
        } else {
            run_silent();
        }
        return;
    }

    let positional: Vec<&str> = args.iter().skip(1).filter(|a| !a.starts_with('-')).map(|s| s.as_str()).collect();

    let cmd = positional.first().copied();

    if matches!(cmd, None | Some("config") | Some("gui")) && !watch && !has(&["--help", "-h"]) {
        if let Err(e) = gui::run(has(&["--demo"])) {
            dlog!("could not open the window: {e}");
            show_error(&format!(
                "Could not open the rzr panel:\n{e}\n\nThe panel needs Microsoft Edge WebView2. If it is missing, install it from https://go.microsoft.com/fwlink/p/?LinkId=2124703\n\nMeanwhile, \"rzr --watch\" and \"rzr apply\" still work."
            ));
            std::process::exit(1);
        }
        return;
    }

    attach_console();
    println!();
    if has(&["--help", "-h"]) || cmd == Some("help") {
        print_help();
    } else if watch || cmd == Some("watch") {
        run_watch(false);
    } else if cmd == Some("apply") {
        run_apply();
    } else if cmd == Some("import") {
        run_import(positional.get(1).copied());
    } else if cmd == Some("diagnose") {
        run_diagnose(&args);
    } else {
        eprintln!("Unknown command: {}", cmd.unwrap_or_default());
        eprintln!("Use 'rzr help' for usage.");
        std::process::exit(1);
    }
}

/// Error dialog for when the panel can't open (there's no console to print to).
#[cfg(windows)]
fn show_error(text: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), &HSTRING::from("rzr"), MB_OK | MB_ICONERROR);
    }
}

#[cfg(not(windows))]
fn show_error(text: &str) {
    eprintln!("rzr: {text}");
}

fn print_help() {
    println!("rzr - Razer BlackShark V2 Pro (2023) control panel");
    println!();
    println!("Controls the headset (EQ, mic monitoring, auto power-off, Do Not");
    println!("Disturb) over USB HID and THX Spatial Audio, replacing Razer Synapse.");
    println!();
    println!("Usage:");
    println!("  rzr                    Open the control panel");
    println!("  rzr apply              Apply the active profile to the headset");
    println!("  rzr import FILE        Import profiles from a Synapse .synapse4 export");
    println!("  rzr --watch            Watch for headset and apply on connect");
    println!("  rzr --silent           Apply silently (no output, for startup)");
    println!("  rzr --silent --watch   Watch silently (best for startup)");
    println!("  rzr diagnose [MODEL] [--seconds N]");
    println!("                         Read-only report for a headset model, to send");
    println!("                         for support. MODEL: {}", model_ids().join(", "));
    println!("  --debug                Also write debug.log (every HID frame)");
    println!("  rzr help               Show this help");
    println!();
    println!("Settings are stored in {}", Config::path().display());
}

fn run_silent() {
    let cfg = Config::load();
    if !cfg.headset_model.supported() {
        std::process::exit(1);
    }
    let dev = match device::Device::open(cfg.wait_timeout_ms) {
        Ok(d) => d,
        Err(_) => std::process::exit(1),
    };
    if let Err(e) = dev.apply_profile(cfg.profile()) {
        dlog!("apply failed: {e}");
    }
}

fn run_watch(silent: bool) {
    if !instance::acquire_watcher() {
        if !silent {
            eprintln!("rzr: another instance is already running.");
        }
        std::process::exit(0);
    }

    let poll_interval = Duration::from_secs(5);
    let mut dev: Option<device::Device> = None;
    let mut log = connlog::ConnLog::new("background");
    let mut was_connected = false;
    let mut applied = false;
    // Consecutive failed link checks; one dropped reply isn't a disconnect.
    let mut misses = 0;
    let mut next_check = Instant::now();
    let mut thx = ThxSync::default();
    winaudio::com_init();

    if !silent {
        println!("rzr: watching for headset (Ctrl+C to stop)");
        println!("  Connection drops are logged to {}", connlog::path().display());
    }

    loop {
        // Keep the dongle open so the headset's own link events reach us:
        // they catch drops too short for the periodic check.
        let Some(d) = dev.as_ref() else {
            // A model rzr doesn't control yet: nothing to do but wait for
            // the user to pick a supported one in the panel.
            if !Config::load().headset_model.supported() {
                thread::sleep(poll_interval);
                continue;
            }
            match device::Device::open(0) {
                Ok(d) => {
                    dev = Some(d);
                    next_check = Instant::now();
                }
                Err(_) => {
                    if was_connected {
                        log.update(false, "dongle not found");
                        if !silent {
                            println!("  Dongle not found, waiting...");
                        }
                    }
                    was_connected = false;
                    applied = false;
                    thread::sleep(poll_interval);
                }
            }
            continue;
        };

        match d.take_events(250) {
            Ok(events) => {
                if applied && events.iter().any(|e| e.preset().is_some()) {
                    follow_preset(d, &mut thx);
                }
                for link in events.iter().filter_map(|e| e.link()) {
                    log.update(link, "headset event");
                    if link {
                        next_check = Instant::now(); // re-apply right away
                    } else {
                        if !silent && was_connected {
                            println!("  Headset disconnected, waiting for reconnect...");
                        }
                        was_connected = false;
                        applied = false;
                        misses = 0;
                    }
                }
            }
            Err(_) => {
                dev = None;
                continue;
            }
        }

        if Instant::now() < next_check {
            continue;
        }
        next_check = Instant::now() + poll_interval;

        // Pick up changes made in the panel (model, debug log, profile).
        let cfg = Config::load();
        debuglog::set_enabled(cfg.debug_log);
        if !cfg.headset_model.supported() {
            dev = None;
            was_connected = false;
            applied = false;
            continue;
        }
        thx.sync_mic(&cfg.profile().mic);

        let connected = match d.link_status() {
            Ok(c) => c,
            // The panel is mid-sequence.
            Err(e) if e == device::BUSY => continue,
            Err(_) => {
                dev = None;
                continue;
            }
        };

        misses = if connected { 0 } else { misses + 1 };
        if was_connected && !connected && misses < 2 {
            continue;
        }
        log.update(connected, "periodic check");

        if connected && !applied {
            // Headset just connected (or first detection).
            if !silent {
                println!("  Headset connected, applying profile \"{}\"...", cfg.profile().name);
            }
            let result = d.apply_profile(cfg.profile());
            if let Err(e) = &result {
                dlog!("apply failed: {e}");
            }
            // Busy = the panel was sending at the same time; retry next check.
            let busy = matches!(&result, Err(e) if e == device::BUSY);
            if !silent {
                if let Some(batt) = d.get_battery() {
                    println!("  Battery: {}%", batt);
                }
                match result {
                    Ok(()) => println!("  Done!"),
                    Err(e) => println!("  Failed: {e}"),
                }
            }
            if !busy {
                applied = true;
            }
        }

        if was_connected && !connected {
            // Headset disconnected — reset so we re-apply on next connect
            if !silent {
                println!("  Headset disconnected, waiting for reconnect...");
            }
            applied = false;
        }

        was_connected = connected;
    }
}

/// The headset's EQ button switched presets while no panel is open: keep the
/// profile and THX with it, as the panel does (ADR 0006). The event only says
/// something changed; the preset read afterwards decides (a select of ours can
/// briefly land elsewhere and send one too).
fn follow_preset(d: &device::Device, thx: &mut ThxSync) {
    if instance::panel_open() {
        return; // the panel follows it
    }
    let Some(preset) = d.get_preset() else { return };
    let mut cfg = Config::load();
    if cfg.profile().active_preset() == preset {
        return;
    }
    dlog!("background: EQ button, the headset is on {preset:?}");
    cfg.profile_mut().select_preset(preset);
    if let Err(e) = cfg.save() {
        dlog!("background: {e}");
    }
    let p = cfg.profile();
    thx.set_eq(&ThxEq::new(p.active_preset(), p.active_curve()));
}

/// The background watcher's link to the THX service.
///
/// Keeps THX's microphone enhancements as the profile has them: THX forgets
/// them when its service restarts (e.g. after a reboot), and Synapse isn't
/// there to send them again (ADR 0007). They're sent when the profile changes
/// and whenever the connection to the service is (re)made, not on every
/// check, so an open Synapse isn't fought over them.
#[derive(Default)]
struct ThxSync {
    service: Option<thx::Service>,
    sent: Option<MicSettings>,
}

impl ThxSync {
    /// The service, connecting if needed. None: THX isn't there (yet: the
    /// service starts with Windows, maybe after us).
    fn service(&mut self) -> Option<&thx::Service> {
        if self.service.as_ref().is_some_and(|s| !s.alive()) {
            dlog!("background: lost the connection to the THX service");
            self.service = None;
        }
        if self.service.is_none() {
            self.service = Some(thx::Service::connect().ok()?);
            self.sent = None;
        }
        self.service.as_ref()
    }

    fn set_eq(&mut self, eq: &ThxEq) {
        let Some(service) = self.service() else { return };
        match service.set_eq(eq) {
            Ok(()) => dlog!("background: THX {eq:?}"),
            Err(e) => {
                dlog!("background: THX equalizer: {e}");
                self.service = None;
            }
        }
    }

    fn sync_mic(&mut self, want: &MicSettings) {
        if self.service().is_none() || self.sent.as_ref() == Some(want) {
            return;
        }
        let Some(service) = &self.service else { return };
        match service.set_mic(want) {
            Ok(()) => {
                dlog!("background: microphone enhancements sent to THX");
                self.sent = Some(*want);
            }
            Err(e) => {
                dlog!("background: microphone enhancements: {e}");
                // Reconnect and try again on the next check.
                self.service = None;
            }
        }
    }
}

fn run_apply() {
    let cfg = Config::load();
    if !cfg.headset_model.supported() {
        eprintln!("rzr: {} is not supported yet; see 'rzr diagnose'.", cfg.headset_model.name());
        std::process::exit(1);
    }

    print!("rzr: waiting for device...");
    io::stdout().flush().ok();

    let dev = match device::Device::open(cfg.wait_timeout_ms) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("\nError: {e}");
            std::process::exit(1);
        }
    };

    println!(" found {}", dev.product);

    if !dev.is_headset_connected() {
        eprintln!("  Headset is off or out of range.");
        std::process::exit(1);
    }

    if let Some(batt) = dev.get_battery() {
        println!("  Battery: {}%", batt);
    }

    print!("  Applying profile \"{}\"...", cfg.profile().name);
    io::stdout().flush().ok();

    match dev.apply_profile(cfg.profile()) {
        Ok(()) => println!(" done!"),
        Err(e) => {
            eprintln!(" failed: {e}");
            std::process::exit(1);
        }
    }
}

fn run_import(path: Option<&str>) {
    let Some(path) = path else {
        eprintln!("Usage: rzr import FILE.synapse4");
        std::process::exit(1);
    };
    let profiles = match synapse::import_file(std::path::Path::new(path)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    let mut cfg = Config::load();
    for mut p in profiles {
        p.name = cfg.unique_name(&p.name);
        println!("  Imported profile \"{}\"", p.name);
        cfg.profiles.push(p);
    }
    cfg.active = cfg.profiles.len() - 1;
    match cfg.save() {
        Ok(()) => println!("  Saved. Active profile: \"{}\"", cfg.profile().name),
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn model_ids() -> Vec<String> {
    models::HeadsetModel::ALL
        .iter()
        .filter_map(|m| serde_json::to_value(m).ok()?.as_str().map(str::to_string))
        .collect()
}

/// `rzr diagnose [MODEL] [--seconds N]`: the read-only report from Settings.
fn run_diagnose(args: &[String]) {
    let mut model = Config::load().headset_model;
    let mut listen = diagnostics::LISTEN;
    let start = args.iter().position(|a| a == "diagnose").map_or(args.len(), |i| i + 1);
    let mut rest = args[start..].iter();
    while let Some(arg) = rest.next() {
        if arg == "--seconds" {
            match rest.next().and_then(|s| s.parse::<u64>().ok()) {
                Some(s) => listen = Duration::from_secs(s.min(600)),
                None => {
                    eprintln!("Usage: rzr diagnose [MODEL] [--seconds N]");
                    std::process::exit(1);
                }
            }
        } else if arg.starts_with('-') {
            // --debug and the like, handled in main().
        } else if let Ok(m) = serde_json::from_value(serde_json::Value::from(arg.as_str())) {
            model = m;
        } else {
            eprintln!("Unknown model: {arg}. Models: {}", model_ids().join(", "));
            std::process::exit(1);
        }
    }
    println!("rzr: diagnostics for {} (read-only)", model.name());
    let result = diagnostics::run(model, listen, &|p| match p {
        diagnostics::Progress::Step(step) => println!("  {step}"),
        diagnostics::Progress::Listening { left, reports } => {
            print!(
                "\r  Listening: turn the headset off and on, press its buttons... {left:3} s left, {reports} reports "
            );
            io::stdout().flush().ok();
        }
    });
    println!();
    match result {
        Ok(report) => {
            for line in &report.summary {
                println!("  {line}");
            }
            println!("  Report saved to {}", report.path.display());
            println!("  Send it with an issue: {}", gui::ISSUES_URL);
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
