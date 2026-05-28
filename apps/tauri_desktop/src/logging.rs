use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{LevelFilter, Metadata, Record};

const DESKTOP_LOG_FILE_NAME: &str = "snap_pin_desktop.log";
static DESKTOP_LOGGER: DesktopFileLogger = DesktopFileLogger {
    file: OnceLock::new(),
};

struct DesktopFileLogger {
    file: OnceLock<Mutex<File>>,
}

impl log::Log for DesktopFileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let line = format!(
            "[{} pid={}] {:<5} {} - {}",
            log_timestamp(),
            std::process::id(),
            record.level(),
            record.target(),
            record.args()
        );

        if let Some(file) = self.file.get()
            && let Ok(mut file) = file.lock()
        {
            let _ = writeln!(file, "{line}");
        }

        if cfg!(debug_assertions) {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(stderr, "{line}");
        }
    }

    fn flush(&self) {
        if let Some(file) = self.file.get()
            && let Ok(mut file) = file.lock()
        {
            let _ = file.flush();
        }
    }
}

pub fn init() {
    let path = std::env::temp_dir().join(DESKTOP_LOG_FILE_NAME);
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => {
            let _ = DESKTOP_LOGGER.file.set(Mutex::new(file));
            if log::set_logger(&DESKTOP_LOGGER).is_ok() {
                log::set_max_level(configured_level());
            }
            log::info!("logging initialized at {}", path.display());
        }
        Err(error) => {
            eprintln!("failed to initialize snap pin desktop log: {error}");
        }
    }
}

fn configured_level() -> LevelFilter {
    match std::env::var("SNAP_PIN_LOG")
        .ok()
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("off") => LevelFilter::Off,
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

fn log_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", now.as_secs(), now.subsec_millis())
}
