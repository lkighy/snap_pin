use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

pub use platform_api::{GlobalHotkey, HotkeyEventSink, HotkeyRegistration, HotkeyToken};

use crate::PlatformError;

pub struct HotkeyListener {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl HotkeyListener {
    pub fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for HotkeyListener {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl HotkeyToken for HotkeyListener {}

pub fn listen_for_hotkey<F>(
    registration: HotkeyRegistration,
    mut on_triggered: F,
) -> Result<HotkeyListener, PlatformError>
where
    F: FnMut(HotkeyRegistration) + Send + 'static,
{
    let (ready_tx, ready_rx) = mpsc::channel();
    let (stop_tx, stop_rx) = mpsc::channel();

    let thread = thread::Builder::new()
        .name(format!("snap-pin-hotkey-{}", registration.id))
        .spawn(move || {
            platform::run_message_loop(registration, stop_rx, ready_tx, &mut on_triggered);
        })
        .map_err(|error| PlatformError::new("hotkey_thread_failed", error.to_string()))?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(HotkeyListener {
            stop: Some(stop_tx),
            thread: Some(thread),
        }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            let _ = thread.join();
            Err(PlatformError::new(
                "hotkey_thread_failed",
                "hotkey listener exited before registration completed",
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedHotkey {
    pub modifiers: u32,
    pub virtual_key: u32,
}

pub fn parse_accelerator(accelerator: &str) -> Result<ParsedHotkey, PlatformError> {
    let mut modifiers = 0;
    let mut key = None;

    for raw_part in accelerator
        .replace('-', "+")
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let part = raw_part.to_ascii_lowercase();
        match part.as_str() {
            "ctrl" | "control" => modifiers |= platform::MOD_CONTROL,
            "shift" => modifiers |= platform::MOD_SHIFT,
            "alt" | "option" => modifiers |= platform::MOD_ALT,
            "win" | "cmd" | "meta" | "super" => modifiers |= platform::MOD_WIN,
            "printscreen" | "print_screen" | "prtsc" | "prt_sc" => {
                key = Some(platform::VK_SNAPSHOT)
            }
            "esc" | "escape" => key = Some(platform::VK_ESCAPE),
            "enter" | "return" => key = Some(platform::VK_RETURN),
            "space" => key = Some(platform::VK_SPACE),
            "tab" => key = Some(platform::VK_TAB),
            _ => {
                let upper = raw_part.to_ascii_uppercase();
                if upper.len() == 1 {
                    let byte = upper.as_bytes()[0];
                    if byte.is_ascii_alphanumeric() {
                        key = Some(byte as u32);
                        continue;
                    }
                }

                if let Some(number) = upper
                    .strip_prefix('F')
                    .and_then(|value| value.parse::<u32>().ok())
                {
                    if (1..=24).contains(&number) {
                        key = Some(platform::VK_F1 + number - 1);
                        continue;
                    }
                }

                return Err(PlatformError::new(
                    "invalid_hotkey",
                    format!("unsupported hotkey token '{raw_part}'"),
                ));
            }
        }
    }

    let Some(virtual_key) = key else {
        return Err(PlatformError::new(
            "invalid_hotkey",
            format!("hotkey '{accelerator}' does not include a key"),
        ));
    };

    Ok(ParsedHotkey {
        modifiers: modifiers | platform::MOD_NOREPEAT,
        virtual_key,
    })
}

#[cfg(windows)]
mod platform {
    use std::sync::mpsc::{Receiver, Sender};
    use std::thread;

    use windows_sys::Win32::Foundation::{LPARAM, WPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT as WIN_MOD_ALT, MOD_CONTROL as WIN_MOD_CONTROL, MOD_NOREPEAT as WIN_MOD_NOREPEAT,
        MOD_SHIFT as WIN_MOD_SHIFT, MOD_WIN as WIN_MOD_WIN, RegisterHotKey, UnregisterHotKey,
        VK_ESCAPE as WIN_VK_ESCAPE, VK_F1 as WIN_VK_F1, VK_RETURN as WIN_VK_RETURN,
        VK_SNAPSHOT as WIN_VK_SNAPSHOT, VK_SPACE as WIN_VK_SPACE, VK_TAB as WIN_VK_TAB,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PM_REMOVE, PeekMessageW, PostThreadMessageW,
        TranslateMessage, WM_HOTKEY, WM_QUIT,
    };

    use super::{HotkeyRegistration, parse_accelerator};
    use crate::PlatformError;

    pub const MOD_ALT: u32 = WIN_MOD_ALT;
    pub const MOD_CONTROL: u32 = WIN_MOD_CONTROL;
    pub const MOD_NOREPEAT: u32 = WIN_MOD_NOREPEAT;
    pub const MOD_SHIFT: u32 = WIN_MOD_SHIFT;
    pub const MOD_WIN: u32 = WIN_MOD_WIN;
    pub const VK_ESCAPE: u32 = WIN_VK_ESCAPE as u32;
    pub const VK_F1: u32 = WIN_VK_F1 as u32;
    pub const VK_RETURN: u32 = WIN_VK_RETURN as u32;
    pub const VK_SNAPSHOT: u32 = WIN_VK_SNAPSHOT as u32;
    pub const VK_SPACE: u32 = WIN_VK_SPACE as u32;
    pub const VK_TAB: u32 = WIN_VK_TAB as u32;

    const HOTKEY_ID: i32 = 0x5350;

    pub fn run_message_loop<F>(
        registration: HotkeyRegistration,
        stop_rx: Receiver<()>,
        ready_tx: Sender<Result<(), PlatformError>>,
        on_triggered: &mut F,
    ) where
        F: FnMut(HotkeyRegistration),
    {
        let parsed = match parse_accelerator(&registration.accelerator) {
            Ok(parsed) => parsed,
            Err(error) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
        };

        // SAFETY: the listener owns this thread message queue. Calling PeekMessageW
        // ensures it exists before another thread posts the stop signal.
        let thread_id = unsafe {
            let mut bootstrap = MSG::default();
            let _ = PeekMessageW(&mut bootstrap, std::ptr::null_mut(), 0, 0, PM_REMOVE);
            GetCurrentThreadId()
        };

        // SAFETY: RegisterHotKey is called with no HWND so the hotkey is bound to
        // this message queue. The id is private to this listener thread.
        let registered = unsafe {
            RegisterHotKey(
                std::ptr::null_mut(),
                HOTKEY_ID,
                parsed.modifiers,
                parsed.virtual_key,
            )
        };

        if registered == 0 {
            let _ = ready_tx.send(Err(PlatformError::new(
                "hotkey_registration_failed",
                format!("failed to register hotkey '{}'", registration.accelerator),
            )));
            return;
        }

        let stopper = thread::spawn(move || {
            let _ = stop_rx.recv();
            // SAFETY: thread_id is captured from the running listener thread and
            // WM_QUIT is the documented way to terminate GetMessageW loops.
            unsafe {
                PostThreadMessageW(thread_id, WM_QUIT, 0 as WPARAM, 0 as LPARAM);
            }
        });

        let _ = ready_tx.send(Ok(()));

        let mut message = MSG::default();
        loop {
            // SAFETY: message points to valid storage for the duration of this call.
            let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
            if result <= 0 {
                break;
            }

            if message.message == WM_HOTKEY && message.wParam == HOTKEY_ID as WPARAM {
                on_triggered(registration.clone());
                continue;
            }

            // SAFETY: message was produced by GetMessageW and can be translated
            // and dispatched through the normal Win32 message pump.
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        // SAFETY: this unregisters the hotkey id registered above on this thread.
        unsafe {
            UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
        }

        if stopper.is_finished() {
            let _ = stopper.join();
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::sync::mpsc::{Receiver, Sender};

    use super::HotkeyRegistration;
    use crate::PlatformError;

    pub const MOD_ALT: u32 = 1;
    pub const MOD_CONTROL: u32 = 2;
    pub const MOD_NOREPEAT: u32 = 16384;
    pub const MOD_SHIFT: u32 = 4;
    pub const MOD_WIN: u32 = 8;
    pub const VK_ESCAPE: u32 = 27;
    pub const VK_F1: u32 = 112;
    pub const VK_RETURN: u32 = 13;
    pub const VK_SNAPSHOT: u32 = 44;
    pub const VK_SPACE: u32 = 32;
    pub const VK_TAB: u32 = 9;

    pub fn run_message_loop<F>(
        _registration: HotkeyRegistration,
        _stop_rx: Receiver<()>,
        ready_tx: Sender<Result<(), PlatformError>>,
        _on_triggered: &mut F,
    ) where
        F: FnMut(HotkeyRegistration),
    {
        let _ = ready_tx.send(Err(PlatformError::new(
            "unsupported_platform",
            "global hotkeys are currently implemented only on Windows",
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::parse_accelerator;

    #[test]
    fn parses_common_accelerator() {
        let parsed = parse_accelerator("Ctrl+Shift+A").expect("hotkey should parse");

        assert_ne!(parsed.modifiers & 2, 0);
        assert_ne!(parsed.modifiers & 4, 0);
        assert_eq!(parsed.virtual_key, b'A' as u32);
    }

    #[test]
    fn rejects_missing_key() {
        let error = parse_accelerator("Ctrl+Shift").expect_err("key is required");

        assert_eq!(error.code, "invalid_hotkey");
    }
}
