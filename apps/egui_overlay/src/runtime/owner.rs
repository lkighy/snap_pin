use std::thread;
use std::time::Duration;

const OWNER_WATCHDOG_INTERVAL: Duration = Duration::from_millis(1_000);

pub(crate) fn start_owner_watchdog(
    owner_pid: Option<u32>,
    owner_process_is_alive: impl Fn(u32) -> bool + Send + 'static,
) {
    let Some(owner_pid) = owner_pid.filter(|pid| *pid > 0) else {
        return;
    };

    thread::spawn(move || {
        log::info!("overlay owner watchdog started owner_pid={owner_pid}");
        loop {
            thread::sleep(OWNER_WATCHDOG_INTERVAL);
            if owner_process_is_alive(owner_pid) {
                continue;
            }

            log::info!("overlay owner process exited; shutting down owner_pid={owner_pid}");
            std::process::exit(0);
        }
    });
}
