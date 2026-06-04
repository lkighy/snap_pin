use std::path::PathBuf;

use crate::PlatformError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    Text(String),
    ImageRgba {
        width: usize,
        height: usize,
        bytes: Vec<u8>,
    },
    Files(Vec<PathBuf>),
}

pub fn read_clipboard_payload() -> Result<ClipboardPayload, PlatformError> {
    platform::read_clipboard_payload()
}

#[cfg(windows)]
mod platform {
    use std::path::PathBuf;

    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Ole::CF_HDROP;
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    use super::ClipboardPayload;
    use crate::PlatformError;

    pub fn read_clipboard_payload() -> Result<ClipboardPayload, PlatformError> {
        let _clipboard = ClipboardGuard::open()?;

        if has_format(CF_HDROP as u32) {
            let files = read_hdrop_files()?;
            if !files.is_empty() {
                return Ok(ClipboardPayload::Files(files));
            }
        }

        drop(_clipboard);

        let mut clipboard = arboard::Clipboard::new().map_err(|error| {
            PlatformError::new(
                "clipboard_open_failed",
                format!("failed to open clipboard: {error}"),
            )
        })?;

        if let Ok(image) = clipboard.get_image() {
            return Ok(ClipboardPayload::ImageRgba {
                width: image.width,
                height: image.height,
                bytes: image.bytes.into_owned(),
            });
        }

        if let Ok(text) = clipboard.get_text() {
            if !text.trim().is_empty() {
                return Ok(ClipboardPayload::Text(text));
            }
        }

        Err(PlatformError::new(
            "clipboard_empty",
            "clipboard does not contain files, an image, or text",
        ))
    }

    struct ClipboardGuard;

    impl ClipboardGuard {
        fn open() -> Result<Self, PlatformError> {
            // SAFETY: passing a null owner opens the process clipboard for the
            // current thread. The guard closes it on every return path.
            let opened = unsafe { OpenClipboard(std::ptr::null_mut()) };
            if opened == 0 {
                return Err(PlatformError::new(
                    "clipboard_open_failed",
                    "failed to open clipboard",
                ));
            }
            Ok(Self)
        }
    }

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            // SAFETY: this balances a successful OpenClipboard in ClipboardGuard::open.
            unsafe {
                CloseClipboard();
            }
        }
    }

    fn has_format(format: u32) -> bool {
        // SAFETY: IsClipboardFormatAvailable only inspects clipboard metadata.
        unsafe { IsClipboardFormatAvailable(format) != 0 }
    }

    fn read_hdrop_files() -> Result<Vec<PathBuf>, PlatformError> {
        // SAFETY: CF_HDROP data is owned by the clipboard while it remains open.
        let handle = unsafe { GetClipboardData(CF_HDROP as u32) };
        if handle.is_null() {
            return Err(PlatformError::new(
                "clipboard_files_unavailable",
                "clipboard reported files but did not return CF_HDROP data",
            ));
        }

        let hdrop = handle as HANDLE;
        // SAFETY: DragQueryFileW with u32::MAX returns the number of file paths
        // in the HDROP handle.
        let count = unsafe { DragQueryFileW(hdrop, u32::MAX, std::ptr::null_mut(), 0) };
        let mut files = Vec::with_capacity(count as usize);

        for index in 0..count {
            // SAFETY: querying with a null buffer returns the path length without
            // the trailing null.
            let length = unsafe { DragQueryFileW(hdrop, index, std::ptr::null_mut(), 0) };
            if length == 0 {
                continue;
            }

            let mut buffer = vec![0u16; length as usize + 1];
            // SAFETY: the buffer is valid for length + 1 UTF-16 code units.
            let written =
                unsafe { DragQueryFileW(hdrop, index, buffer.as_mut_ptr(), buffer.len() as u32) };
            if written == 0 {
                continue;
            }

            files.push(PathBuf::from(String::from_utf16_lossy(
                &buffer[..written as usize],
            )));
        }

        Ok(files)
    }
}

#[cfg(not(windows))]
mod platform {
    use super::ClipboardPayload;
    use crate::PlatformError;

    pub fn read_clipboard_payload() -> Result<ClipboardPayload, PlatformError> {
        Err(PlatformError::new(
            "unsupported_platform",
            "clipboard pinning is currently implemented only on Windows",
        ))
    }
}
