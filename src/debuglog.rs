/// Optional debug log (debug.log next to the config), off by default.
///
/// When enabled it records every HID frame sent to and received from the
/// dongle, plus what the app was doing at the time, so a problem on the
/// user's machine can be diagnosed from the file alone. Turned on from
/// AJUSTES, with `--debug`, or with the RZR_DEBUG environment variable.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::config::Config;

/// Past this size the log is moved to debug.old.log and a new one started.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Enabled from the command line or environment: config changes can't turn it off.
static FORCED: AtomicBool = AtomicBool::new(false);
static SOURCE: OnceLock<&'static str> = OnceLock::new();
static WRITE: Mutex<()> = Mutex::new(());

pub fn path() -> PathBuf {
    Config::path().with_file_name("debug.log")
}

/// Name this process in each line ("panel", "segundo plano"...), and honour
/// `--debug` / RZR_DEBUG.
pub fn init(source: &'static str, forced: bool) {
    let _ = SOURCE.set(source);
    if forced || std::env::var_os("RZR_DEBUG").is_some() {
        FORCED.store(true, Ordering::Relaxed);
        set_enabled(true);
    }
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    let on = on || FORCED.load(Ordering::Relaxed);
    if ENABLED.swap(on, Ordering::Relaxed) != on && on {
        write(&format!(
            "=== registro activado: rzr {} ({}, {}) ===",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }
}

/// Append one line (no-op when disabled). Use through `dlog!`.
pub fn write(text: &str) {
    if !enabled() {
        return;
    }
    let path = path();
    let _guard = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&path, path.with_file_name("debug.old.log"));
    }
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let source = SOURCE.get().copied().unwrap_or("rzr");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{stamp} [{source}] {text}");
    }
}

/// Hex dump of the meaningful part of a report (trailing zeros dropped,
/// keeping at least the 13-byte header).
pub fn hex(buf: &[u8]) -> String {
    let end = buf.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1).max(13.min(buf.len()));
    buf[..end].iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// `dlog!("fmt", args...)`: write a debug line; arguments are only
/// formatted when the log is on.
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        if $crate::debuglog::enabled() {
            $crate::debuglog::write(&format!($($arg)*));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_trims_padding() {
        let mut pkt = [0u8; 64];
        pkt[..3].copy_from_slice(&[0x02, 0x80, 0x09]);
        pkt[13] = 0xFF;
        assert_eq!(hex(&pkt), "02 80 09 00 00 00 00 00 00 00 00 00 00 FF");
        assert_eq!(hex(&[0u8; 64]).split(' ').count(), 13);
    }
}
