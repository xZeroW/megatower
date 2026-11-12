//! The game's log file: everything the console shows, plus panics with a
//! backtrace, so a crash on a machine without a terminal can still be read.
//!
//! `%APPDATA%\megatower\logs\megatower.log` on Windows,
//! `~/.local/share/megatower/logs/megatower.log` elsewhere (`$XDG_DATA_HOME`
//! is honoured). The previous run's log is kept as `megatower.previous.log`.
//! `RUST_LOG` (for example `info,bevy_winit=debug,wgpu=warn`) sets how much
//! is written.

use bevy::log::BoxedLayer;
use bevy::prelude::*;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const LOG: &str = "megatower.log";
const PREVIOUS: &str = "megatower.previous.log";

/// Where the logs go, from the environment (passed in so it can be tested).
fn dir_from(
    appdata: Option<OsString>,
    xdg_data: Option<OsString>,
    home: Option<OsString>,
) -> PathBuf {
    if let Some(appdata) = appdata {
        return Path::new(&appdata).join("megatower").join("logs");
    }
    xdg_data
        .map(PathBuf::from)
        .or_else(|| home.map(|h| Path::new(&h).join(".local/share")))
        .unwrap_or_else(|| ".".into())
        .join("megatower")
        .join("logs")
}

pub fn log_dir() -> PathBuf {
    dir_from(
        std::env::var_os("APPDATA"),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    )
}

/// The open log, shared by the tracing layer and the panic hook.
static FILE: OnceLock<Mutex<File>> = OnceLock::new();

/// Moves the last log aside and opens a new one. `None` when the folder or
/// file cannot be made: the game then logs to the console only.
fn open(dir: &Path) -> Option<&'static Mutex<File>> {
    if let Some(file) = FILE.get() {
        return Some(file);
    }
    fs::create_dir_all(dir).ok()?;
    let current = dir.join(LOG);
    if current.exists() {
        let _ = fs::rename(&current, dir.join(PREVIOUS));
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(current)
        .ok()?;
    Some(FILE.get_or_init(|| Mutex::new(file)))
}

fn write_line(text: &str) {
    if let Some(file) = FILE.get()
        && let Ok(mut file) = file.lock()
    {
        let _ = writeln!(file, "{text}");
        let _ = file.flush();
    }
}

/// The file layer for `LogPlugin::custom_layer`. Every line is written as it
/// happens, so the last thing logged before a crash is in the file.
pub fn layer(_app: &mut App) -> Option<BoxedLayer> {
    let file = open(&log_dir())?;
    write_line(&format!(
        "megatower {} on {} {}, log {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        log_dir().join(LOG).display()
    ));
    let writer = move || FileWriter(file);
    Some(Box::new(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(writer),
    ))
}

struct FileWriter(&'static Mutex<File>);

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.0.lock() {
            Ok(mut file) => file.write(buf),
            Err(_) => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.0.lock() {
            Ok(mut file) => file.flush(),
            Err(_) => Ok(()),
        }
    }
}

/// Writes panics, with where they happened and a backtrace, to the log and
/// then to the console as before. Call before the app is built.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        write_line(&format!(
            "PANIC in thread '{}': {info}\n{}",
            thread.name().unwrap_or("<unnamed>"),
            std::backtrace::Backtrace::force_capture()
        ));
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logs_go_to_appdata_on_windows_and_the_data_dir_elsewhere() {
        let os = |s: &str| Some(OsString::from(s));
        assert_eq!(
            dir_from(os("C:/Users/me/AppData/Roaming"), None, None),
            Path::new("C:/Users/me/AppData/Roaming/megatower/logs")
        );
        assert_eq!(
            dir_from(None, os("/data"), os("/home/me")),
            Path::new("/data/megatower/logs")
        );
        assert_eq!(
            dir_from(None, None, os("/home/me")),
            Path::new("/home/me/.local/share/megatower/logs")
        );
    }

    #[test]
    fn a_new_log_replaces_the_last_one_which_is_kept() {
        let dir = std::env::temp_dir().join(format!("megatower-log-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(LOG), "old run").unwrap();
        assert!(open(&dir).is_some());
        write_line("new run");
        assert_eq!(fs::read_to_string(dir.join(PREVIOUS)).unwrap(), "old run");
        assert!(
            fs::read_to_string(dir.join(LOG))
                .unwrap()
                .contains("new run")
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
