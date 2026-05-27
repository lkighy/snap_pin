use std::path::PathBuf;

use crate::PlatformError;

pub fn prompt_save_png_path(default_file_name: &str) -> Result<Option<PathBuf>, PlatformError> {
    platform::prompt_save_png_path(default_file_name, None)
}

pub fn prompt_save_png_path_with_owner(
    default_file_name: &str,
    owner_hwnd: Option<isize>,
) -> Result<Option<PathBuf>, PlatformError> {
    platform::prompt_save_png_path(default_file_name, owner_hwnd)
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::ptr::{null, null_mut};

    use windows::Win32::Foundation::{
        E_ABORT, ERROR_CANCELLED, HWND as WindowsHwnd, RPC_E_CHANGED_MODE, S_FALSE,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
        CoInitializeEx, CoTaskMemFree, CoUninitialize,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST,
        FileSaveDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
    };
    use windows::core::{HRESULT, HSTRING, PCWSTR};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        CommDlgExtendedError, GetSaveFileNameW, OFN_ENABLESIZING, OFN_EXPLORER, OFN_HIDEREADONLY,
        OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetClassInfoW, GetCursorPos,
        GetSystemMetrics, HWND_TOPMOST, IsWindow, IsWindowVisible, RegisterClassW,
        SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, WNDCLASSW, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP,
    };

    use crate::PlatformError;

    const MAX_PATH_BUFFER: usize = 32_768;

    pub fn prompt_save_png_path(
        default_file_name: &str,
        owner_hwnd: Option<isize>,
    ) -> Result<Option<PathBuf>, PlatformError> {
        log::info!(
            "save dialog requested default_file_name={} owner={:?}",
            default_file_name,
            owner_hwnd
        );
        match prompt_save_png_path_modern(default_file_name, owner_hwnd) {
            Ok(path) => return Ok(path),
            Err(error) => {
                log::warn!("modern save dialog failed, falling back to legacy dialog: {error}");
            }
        }

        prompt_save_png_path_legacy(default_file_name, owner_hwnd)
    }

    fn prompt_save_png_path_modern(
        default_file_name: &str,
        owner_hwnd: Option<isize>,
    ) -> Result<Option<PathBuf>, PlatformError> {
        log::info!("opening modern save dialog");
        let _com = ComApartment::initialize()?;
        let dialog: IFileSaveDialog =
            unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER) }
                .map_err(|error| save_dialog_error("create", error))?;
        let png_name = HSTRING::from("PNG image (*.png)");
        let png_spec = HSTRING::from("*.png");
        let all_name = HSTRING::from("All files (*.*)");
        let all_spec = HSTRING::from("*.*");
        let filters = [
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(png_name.as_ptr()),
                pszSpec: PCWSTR(png_spec.as_ptr()),
            },
            COMDLG_FILTERSPEC {
                pszName: PCWSTR(all_name.as_ptr()),
                pszSpec: PCWSTR(all_spec.as_ptr()),
            },
        ];

        unsafe {
            dialog
                .SetFileTypes(&filters)
                .map_err(|error| save_dialog_error("set file types", error))?;
            dialog
                .SetFileTypeIndex(1)
                .map_err(|error| save_dialog_error("set file type", error))?;
            dialog
                .SetDefaultExtension(&HSTRING::from("png"))
                .map_err(|error| save_dialog_error("set default extension", error))?;
            dialog
                .SetFileName(&HSTRING::from(default_file_name))
                .map_err(|error| save_dialog_error("set file name", error))?;
            dialog
                .SetTitle(&HSTRING::from("Save screenshot"))
                .map_err(|error| save_dialog_error("set title", error))?;
            dialog
                .SetOptions(
                    FOS_OVERWRITEPROMPT | FOS_NOCHANGEDIR | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM,
                )
                .map_err(|error| save_dialog_error("set options", error))?;
        }

        let owner = valid_hwnd(owner_hwnd);
        let fallback_owner = if owner.is_none() {
            Some(DialogOwnerWindow::new(true)?)
        } else {
            None
        };
        let owner = owner.or_else(|| fallback_owner.as_ref().map(|window| window.hwnd));
        log::info!(
            "showing modern save dialog owner={:?} fallback_owner={}",
            owner,
            fallback_owner.is_some()
        );

        match unsafe { dialog.Show(owner.map(WindowsHwnd)) } {
            Ok(()) => {}
            Err(error) if is_dialog_canceled(error.code()) => {
                log::info!("modern save dialog canceled");
                return Ok(None);
            }
            Err(error) => return Err(save_dialog_error("show", error)),
        }

        let result = unsafe { dialog.GetResult() }
            .map_err(|error| save_dialog_error("get result", error))?;
        let path = unsafe { result.GetDisplayName(SIGDN_FILESYSPATH) }
            .map_err(|error| save_dialog_error("get file path", error))?;
        let path_string = unsafe { path.to_string() }
            .map_err(|error| PlatformError::new("save_dialog_failed", error.to_string()))?;
        unsafe {
            CoTaskMemFree(Some(path.as_ptr().cast()));
        }

        if path_string.is_empty() {
            log::info!("modern save dialog returned empty path");
            Ok(None)
        } else {
            log::info!("modern save dialog selected {}", path_string);
            Ok(Some(PathBuf::from(path_string)))
        }
    }

    fn prompt_save_png_path_legacy(
        default_file_name: &str,
        owner_hwnd: Option<isize>,
    ) -> Result<Option<PathBuf>, PlatformError> {
        log::info!("opening legacy save dialog");
        let mut file_buffer = wide_buffer(default_file_name, MAX_PATH_BUFFER);
        let filter = wide_null("PNG image (*.png)\0*.png\0All files (*.*)\0*.*\0");
        let default_extension = wide_null("png");
        let title = wide_null("Save screenshot");
        let owner = valid_hwnd(owner_hwnd);
        let fallback_owner = if owner.is_none() {
            Some(DialogOwnerWindow::new(true)?)
        } else {
            None
        };
        let owner = owner.or_else(|| fallback_owner.as_ref().map(|window| window.hwnd));
        log::info!(
            "showing legacy save dialog owner={:?} fallback_owner={}",
            owner,
            fallback_owner.is_some()
        );

        let mut dialog = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: owner.unwrap_or(null_mut()),
            lpstrFilter: filter.as_ptr(),
            nFilterIndex: 1,
            lpstrFile: file_buffer.as_mut_ptr(),
            nMaxFile: file_buffer.len() as u32,
            lpstrDefExt: default_extension.as_ptr(),
            lpstrTitle: title.as_ptr(),
            Flags: OFN_OVERWRITEPROMPT
                | OFN_NOCHANGEDIR
                | OFN_EXPLORER
                | OFN_PATHMUSTEXIST
                | OFN_HIDEREADONLY
                | OFN_ENABLESIZING,
            ..Default::default()
        };

        let selected = unsafe { GetSaveFileNameW(&mut dialog) };
        if selected == 0 {
            let error = unsafe { CommDlgExtendedError() };
            if error != 0 {
                log::error!("legacy save dialog failed common dialog error=0x{error:04x}");
                return Err(PlatformError::new(
                    "save_dialog_failed",
                    format!("Windows save dialog failed with common dialog error 0x{error:04x}"),
                ));
            }
            log::info!("legacy save dialog canceled");
            return Ok(None);
        }

        let len = file_buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(file_buffer.len());
        if len == 0 {
            log::info!("legacy save dialog returned empty path");
            return Ok(None);
        }

        let path = String::from_utf16_lossy(&file_buffer[..len]);
        log::info!("legacy save dialog selected {}", path);
        Ok(Some(PathBuf::from(path)))
    }

    struct DialogOwnerWindow {
        hwnd: HWND,
    }

    impl DialogOwnerWindow {
        fn new(topmost: bool) -> Result<Self, PlatformError> {
            let class_name = wide_null("SnapPinSaveDialogOwner");
            let title = wide_null("snap pin save dialog owner");
            let instance = unsafe { GetModuleHandleW(null()) };
            if instance.is_null() {
                log::error!("failed to get current module handle for dialog owner");
                return Err(PlatformError::new(
                    "save_dialog_owner_failed",
                    "failed to get current module handle",
                ));
            }

            ensure_owner_class(instance, &class_name);
            let (x, y) = dialog_anchor();
            let ex_style = if topmost {
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST
            } else {
                WS_EX_TOOLWINDOW
            };
            let hwnd = unsafe {
                CreateWindowExW(
                    ex_style,
                    class_name.as_ptr(),
                    title.as_ptr(),
                    WS_POPUP,
                    x,
                    y,
                    1,
                    1,
                    null_mut(),
                    null_mut(),
                    instance,
                    null(),
                )
            };

            if hwnd.is_null() {
                log::error!("failed to create save dialog owner window");
                return Err(PlatformError::new(
                    "save_dialog_owner_failed",
                    "failed to create topmost owner window for save dialog",
                ));
            }

            if topmost {
                unsafe {
                    SetWindowPos(hwnd, HWND_TOPMOST, x, y, 1, 1, SWP_SHOWWINDOW);
                    SetForegroundWindow(hwnd);
                }
            }

            log::info!("created save dialog owner hwnd={hwnd:?} topmost={topmost}");
            Ok(Self { hwnd })
        }
    }

    impl Drop for DialogOwnerWindow {
        fn drop(&mut self) {
            if !self.hwnd.is_null() {
                unsafe {
                    DestroyWindow(self.hwnd);
                }
            }
        }
    }

    fn valid_hwnd(hwnd: Option<isize>) -> Option<HWND> {
        let hwnd = hwnd? as HWND;
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 || unsafe { IsWindowVisible(hwnd) } == 0
        {
            None
        } else {
            Some(hwnd)
        }
    }

    struct ComApartment {
        should_uninitialize: bool,
    }

    impl ComApartment {
        fn initialize() -> Result<Self, PlatformError> {
            let result =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            if result == RPC_E_CHANGED_MODE {
                return Err(PlatformError::new(
                    "save_dialog_failed",
                    "current COM apartment is not STA",
                ));
            }
            if result.is_err() {
                return Err(PlatformError::new(
                    "save_dialog_failed",
                    format!("failed to initialize COM: {}", result.message()),
                ));
            }

            Ok(Self {
                should_uninitialize: result != S_FALSE,
            })
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            if self.should_uninitialize {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }

    fn save_dialog_error(action: &str, error: windows::core::Error) -> PlatformError {
        PlatformError::new(
            "save_dialog_failed",
            format!("Windows save dialog failed to {action}: {error}"),
        )
    }

    fn is_dialog_canceled(code: HRESULT) -> bool {
        code == E_ABORT || code == HRESULT::from_win32(ERROR_CANCELLED.0)
    }

    fn ensure_owner_class(instance: windows_sys::Win32::Foundation::HINSTANCE, class_name: &[u16]) {
        let mut existing = WNDCLASSW::default();
        if unsafe { GetClassInfoW(instance, class_name.as_ptr(), &mut existing) } != 0 {
            return;
        }

        let window_class = WNDCLASSW {
            lpfnWndProc: Some(owner_wnd_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..Default::default()
        };
        unsafe {
            RegisterClassW(&window_class);
        }
    }

    unsafe extern "system" fn owner_wnd_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn dialog_anchor() -> (i32, i32) {
        let mut point = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut point) } != 0 {
            return (point.x, point.y);
        }

        let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        (x + width / 2, y + height / 2)
    }

    fn wide_buffer(value: &str, len: usize) -> Vec<u16> {
        let mut buffer = vec![0; len];
        for (index, unit) in OsStr::new(value)
            .encode_wide()
            .take(len.saturating_sub(1))
            .enumerate()
        {
            buffer[index] = unit;
        }
        buffer
    }

    fn wide_null(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::PathBuf;

    use crate::PlatformError;

    pub fn prompt_save_png_path(
        _default_file_name: &str,
        _owner_hwnd: Option<isize>,
    ) -> Result<Option<PathBuf>, PlatformError> {
        Err(PlatformError::new(
            "unsupported_platform",
            "save file dialogs are currently implemented only on Windows",
        ))
    }
}
