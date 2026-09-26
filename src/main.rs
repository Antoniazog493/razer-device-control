// GUI-subsystem on Windows: no console window flashes on double-click or at
// login. CLI commands attach to the parent console instead.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod connlog;
mod debuglog;
mod device;
mod gui;
mod instance;
mod mic;
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
use worker::Target;

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
        "segundo plano"
    } else if silent || args.len() > 1 && !has(&["--demo", "--debug"]) {
        "cli"
    } else {
        "panel"
    };
    debuglog::init(source, debug);
    debuglog::set_enabled(Config::load().debug_log);
    dlog!("inicio: {:?}", &args[1..]);

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
            dlog!("no se pudo abrir la ventana: {e}");
            show_error(&format!(
                "No se pudo abrir el panel de rzr:\n{e}\n\nEl panel usa Microsoft Edge WebView2. Si falta, instálalo desde https://go.microsoft.com/fwlink/p/?LinkId=2124703\n\nMientras tanto puedes usar «rzr --watch» y «rzr apply»."
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
    println!("rzr - Razer BlackShark V2 Pro control panel");
    println!();
    println!("Controls the headset (EQ, mic monitoring, auto power-off, Do Not");
    println!("Disturb) over USB HID, replacing Razer Synapse.");
    println!();
    println!("Usage:");
    println!("  rzr                    Open the control panel");
    println!("  rzr apply              Apply the active profile to the headset");
    println!("  rzr import FILE        Import profiles from a Synapse .synapse4 export");
    println!("  rzr --watch            Watch for headset and apply on connect");
    println!("  rzr --silent           Apply silently (no output, for startup)");
    println!("  rzr --silent --watch   Watch silently (best for startup)");
    println!("  --debug                Also write debug.log (every HID frame)");
    println!("  rzr help               Show this help");
    println!();
    println!("Settings are stored in {}", Config::path().display());
}

fn run_silent() {
    let cfg = Config::load();
    let dev = match device::Device::open(cfg.wait_timeout_ms) {
        Ok(d) => d,
        Err(_) => std::process::exit(1),
    };
    dev.set_options(device::Options::from_config(&cfg));
    if let Err(e) = dev.apply_profile(cfg.profile()) {
        dlog!("error al aplicar: {e}");
    }
    worker::apply_default_devices(&Target::from_config(&cfg));
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
    let mut log = connlog::ConnLog::new("segundo plano");
    let mut was_connected = false;
    let mut applied = false;
    // Consecutive failed link checks; one dropped reply isn't a disconnect.
    let mut misses = 0;
    let mut next_check = Instant::now();
    let mut mic = MicSync::default();
    winaudio::com_init();

    if !silent {
        println!("rzr: watching for headset (Ctrl+C to stop)");
        println!("  Connection drops are logged to {}", connlog::path().display());
    }

    loop {
        // Keep the dongle open so the headset's own link events reach us:
        // they catch drops too short for the periodic check.
        let Some(d) = dev.as_ref() else {
            match device::Device::open(0) {
                Ok(d) => {
                    dev = Some(d);
                    next_check = Instant::now();
                }
                Err(_) => {
                    if was_connected {
                        log.update(false, "dongle no encontrado");
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
                for link in events.iter().filter_map(|e| e.link()) {
                    log.update(link, "aviso del headset");
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

        // Pick up changes made in the panel (method, debug log).
        let cfg = Config::load();
        debuglog::set_enabled(cfg.debug_log);
        d.set_options(device::Options::from_config(&cfg));
        mic.sync(&cfg.profile().mic);

        let connected = match d.link_status() {
            Ok(c) => c,
            // The panel is mid-sequence (or running the guided test).
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
        log.update(connected, "comprobación periódica");

        if connected && !applied {
            // Headset just connected (or first detection).
            if !silent {
                println!("  Headset connected, applying profile \"{}\"...", cfg.profile().name);
            }
            let result = d.apply_profile(cfg.profile());
            if let Err(e) = &result {
                dlog!("error al aplicar: {e}");
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
                worker::apply_default_devices(&Target::from_config(&cfg));
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

/// Keeps THX's microphone enhancements as the profile has them: THX forgets
/// them when its service restarts (e.g. after a reboot), and Synapse isn't
/// there to send them again (ADR 0007). They're sent when the profile changes
/// and whenever the connection to the service is (re)made, not on every
/// check, so an open Synapse isn't fought over them.
#[derive(Default)]
struct MicSync {
    service: Option<thx::Service>,
    sent: Option<MicSettings>,
}

impl MicSync {
    fn sync(&mut self, want: &MicSettings) {
        if self.service.as_ref().is_some_and(|s| !s.alive()) {
            dlog!("segundo plano: se perdió la conexión con el servicio de THX");
            self.service = None;
        }
        if self.service.is_none() {
            // Not there yet (the service starts with Windows, maybe after us).
            let Ok(s) = thx::Service::connect() else { return };
            self.service = Some(s);
            self.sent = None;
        }
        if self.sent.as_ref() == Some(want) {
            return;
        }
        let Some(service) = &self.service else { return };
        match service.set_mic(want) {
            Ok(()) => {
                dlog!("segundo plano: mejoras del micrófono enviadas a THX");
                self.sent = Some(*want);
            }
            Err(e) => {
                dlog!("segundo plano: mejoras del micrófono: {e}");
                // Reconnect and try again on the next check.
                self.service = None;
            }
        }
    }
}

fn run_apply() {
    let cfg = Config::load();

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
    dev.set_options(device::Options::from_config(&cfg));

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

    worker::apply_default_devices(&Target::from_config(&cfg));
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
