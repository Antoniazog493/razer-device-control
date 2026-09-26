//! Log of headset link drops, kept next to the config as connection.log, to
//! help pin down intermittent disconnects.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use crate::config::Config;

/// Keep the log under this size; older half is dropped when exceeded.
const MAX_BYTES: u64 = 256 * 1024;

pub fn path() -> PathBuf {
    Config::path().with_file_name("connection.log")
}

/// Tracks the link state and writes one line per change.
pub struct ConnLog {
    /// Who is logging ("panel" or "background"), shown on each line.
    source: &'static str,
    state: Option<bool>,
    down_since: Option<Instant>,
}

impl ConnLog {
    pub fn new(source: &'static str) -> Self {
        Self { source, state: None, down_since: None }
    }

    /// Record the current link state; only changes are written.
    /// `how` says what noticed it (event from the headset, periodic check...).
    pub fn update(&mut self, connected: bool, how: &str) {
        if self.state == Some(connected) {
            return;
        }
        let first = self.state.is_none();
        self.state = Some(connected);
        let text = match (connected, first) {
            (true, true) => "Headset connected (log started)".to_string(),
            (true, false) => match self.down_since.take() {
                Some(t) => format!("Headset RECONNECTED after {:.1} s", t.elapsed().as_secs_f32()),
                None => "Headset connected".to_string(),
            },
            (false, _) => {
                self.down_since = Some(Instant::now());
                if first {
                    "Headset not connected (log started)".to_string()
                } else {
                    "Headset DISCONNECTED".to_string()
                }
            }
        };
        append(&format!("{text}  [{how}, {}]", self.source));
    }
}

fn append(text: &str) {
    let path = path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    trim(&path);
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{stamp}  {text}");
    }
}

/// Drop the older half of the log once it grows past MAX_BYTES.
fn trim(path: &PathBuf) {
    let too_big = std::fs::metadata(path).map(|m| m.len() > MAX_BYTES).unwrap_or(false);
    if !too_big {
        return;
    }
    if let Ok(text) = std::fs::read_to_string(path) {
        let lines: Vec<&str> = text.lines().collect();
        let keep = lines[lines.len() / 2..].join("\n");
        let _ = std::fs::write(path, keep + "\n");
    }
}

/// The last `n` lines, newest first.
pub fn recent(n: usize) -> Vec<String> {
    std::fs::read_to_string(path()).map(|t| t.lines().rev().take(n).map(str::to_string).collect()).unwrap_or_default()
}

/// Drops logged today.
pub fn drops_today() -> usize {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    std::fs::read_to_string(path())
        .map(|t| t.lines().filter(|l| l.starts_with(&today) && l.contains("DISCONNECTED")).count())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_drops_with_duration() {
        let dir = std::env::temp_dir().join(format!("rzr-connlog-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("APPDATA");

        let mut log = ConnLog::new("test");
        log.update(true, "start");
        log.update(true, "again"); // no change, not written
        log.update(false, "headset event");
        std::thread::sleep(std::time::Duration::from_millis(20));
        log.update(true, "headset event");

        let lines = recent(10);
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(lines[0].contains("RECONNECTED after"));
        assert!(lines[1].contains("DISCONNECTED"));
        assert!(lines[2].contains("log started"));
        assert_eq!(drops_today(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
