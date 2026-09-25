// GUI-subsystem on Windows: no console window flashes on double-click or at
// login. CLI commands attach to the parent console instead.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod device;
mod gui;
mod instance;
mod protocol;
mod registry;
mod synapse;
mod winaudio;
mod worker;

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use config::Config;
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

    if silent {
        if watch {
            run_watch(true);
        } else {
            run_silent();
        }
        return;
    }

    let positional: Vec<&str> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();

    let cmd = positional.first().copied();

    if matches!(cmd, None | Some("config") | Some("gui")) && !watch && !has(&["--help", "-h"]) {
        if let Err(e) = gui::run(has(&["--demo"])) {
            attach_console();
            eprintln!("rzr: could not open the window: {e}");
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
    let _ = dev.apply_profile(cfg.profile(), cfg.send_legacy_config);
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
    let mut was_connected = false;
    let mut applied = false;
    // Consecutive failed link checks; one dropped reply isn't a disconnect.
    let mut misses = 0;

    if !silent {
        println!("rzr: watching for headset (poll every 5s, Ctrl+C to stop)");
    }

    loop {
        let connected = match device::Device::open(1000) {
            Ok(dev) => {
                let c = dev.is_headset_connected();
                if c && !applied {
                    // Headset just connected (or first detection).
                    // Reload so changes made in the GUI are picked up.
                    let cfg = Config::load();
                    if !silent {
                        println!("  Headset connected, applying profile \"{}\"...", cfg.profile().name);
                    }
                    let result = dev.apply_profile(cfg.profile(), cfg.send_legacy_config);
                    if !silent {
                        if let Some(batt) = dev.get_battery() {
                            println!("  Battery: {}%", batt);
                        }
                        match result {
                            Ok(()) => println!("  Done!"),
                            Err(e) => println!("  Failed: {e}"),
                        }
                    }
                    worker::apply_default_devices(&Target::from_config(&cfg));
                    applied = true;
                }
                c
            }
            Err(_) => false,
        };

        misses = if connected { 0 } else { misses + 1 };
        if was_connected && !connected && misses < 2 {
            thread::sleep(poll_interval);
            continue;
        }

        if was_connected && !connected {
            // Headset disconnected — reset so we re-apply on next connect
            if !silent {
                println!("  Headset disconnected, waiting for reconnect...");
            }
            applied = false;
        }

        was_connected = connected;
        thread::sleep(poll_interval);
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

    if !dev.is_headset_connected() {
        eprintln!("  Headset is off or out of range.");
        std::process::exit(1);
    }

    if let Some(batt) = dev.get_battery() {
        println!("  Battery: {}%", batt);
    }

    print!("  Applying profile \"{}\"...", cfg.profile().name);
    io::stdout().flush().ok();

    match dev.apply_profile(cfg.profile(), cfg.send_legacy_config) {
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
