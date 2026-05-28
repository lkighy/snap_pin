use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{LevelFilter, Metadata, Record};

const OVERLAY_LOG_FILE_NAME: &str = "snap_pin_overlay.log";

static OVERLAY_LOGGER: OverlayFileLogger = OverlayFileLogger {
    file: OnceLock::new(),
};

struct OverlayFileLogger {
    file: OnceLock<Mutex<File>>,
}

impl log::Log for OverlayFileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
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

pub(crate) fn init_logging() {
    let path = std::env::temp_dir().join(OVERLAY_LOG_FILE_NAME);
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => {
            let _ = OVERLAY_LOGGER.file.set(Mutex::new(file));
            if log::set_logger(&OVERLAY_LOGGER).is_ok() {
                log::set_max_level(LevelFilter::Info);
            }
            log::info!("logging initialized at {}", path.display());
        }
        Err(error) => {
            eprintln!("failed to initialize snap pin overlay log: {error}");
        }
    }
}

fn log_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", now.as_secs(), now.subsec_millis())
}
