use std::sync::Mutex;
use std::time::{Duration, Instant};

use platform_api::{
    AppPlatform, CapabilityStatus, Clipboard, ClipboardPayload, FileDialog, GlobalHotkey,
    HotkeyEventSink, HotkeyRegistration, HotkeyToken, ImageData, MonitorInfo, OcrJob, OcrResult,
    PlatformCapabilities, PlatformError, PlatformWindowRef, Rect, ScreenCapture, SharedMemory,
    SharedMemoryCreateRequest, SharedMemoryHandle, SystemOcr, WindowOps,
};

use crate::{
    CaptureBackendHint, CaptureBackendKind, CaptureRequest, CapturedFrame, DxgiCaptureBackend,
    GdiCaptureBackend, WgcCaptureBackend, WindowsCaptureBackend, capture_region,
    create_named_shared_memory, listen_for_hotkey, prompt_folder_path, prompt_save_png_path,
    read_clipboard_payload, read_named_shared_memory, recognize_system_ocr, set_always_on_top,
    set_click_through, virtual_screen_bounds, write_clipboard_payload,
};

const DXGI_FAILURE_COOLDOWN: Duration = Duration::from_secs(30);
static DXGI_SKIP_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Debug, Default)]
pub struct Win32Platform {
    screen_capture: Win32ScreenCapture,
    system_ocr: Win32SystemOcr,
    clipboard: Win32Clipboard,
    global_hotkey: Win32GlobalHotkey,
    window_ops: Win32WindowOps,
    file_dialog: Win32FileDialog,
    shared_memory: Win32SharedMemory,
}

impl AppPlatform for Win32Platform {
    fn capabilities(&self) -> PlatformCapabilities {
        win32_capabilities()
    }

    fn screen_capture(&self) -> &dyn ScreenCapture {
        &self.screen_capture
    }

    fn system_ocr(&self) -> &dyn SystemOcr {
        &self.system_ocr
    }

    fn clipboard(&self) -> &dyn Clipboard {
        &self.clipboard
    }

    fn global_hotkey(&self) -> &dyn GlobalHotkey {
        &self.global_hotkey
    }

    fn window_ops(&self) -> &dyn WindowOps {
        &self.window_ops
    }

    fn file_dialog(&self) -> &dyn FileDialog {
        &self.file_dialog
    }

    fn shared_memory(&self) -> &dyn SharedMemory {
        &self.shared_memory
    }
}

#[derive(Debug, Default)]
struct Win32ScreenCapture;

impl ScreenCapture for Win32ScreenCapture {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        let bounds = virtual_screen_bounds();
        Ok(vec![MonitorInfo {
            id: "virtual-screen".to_owned(),
            name: Some("Virtual screen".to_owned()),
            bounds,
            scale_factor: 1.0,
            primary: true,
        }])
    }

    fn virtual_bounds(&self) -> Result<Rect, PlatformError> {
        Ok(virtual_screen_bounds())
    }

    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        match request
            .backend_hint
            .unwrap_or(CaptureBackendHint::BestAvailable)
        {
            CaptureBackendHint::BestAvailable => capture_with_backends(request),
            CaptureBackendHint::LowLatency => DxgiCaptureBackend.capture(request),
            CaptureBackendHint::Compatibility => GdiCaptureBackend.capture(request),
        }
    }
}

#[derive(Debug, Default)]
struct Win32SystemOcr;

impl SystemOcr for Win32SystemOcr {
    fn availability(&self) -> CapabilityStatus {
        system_ocr_status()
    }

    fn recognize(&self, job: &OcrJob, image: &ImageData) -> Result<OcrResult, PlatformError> {
        recognize_system_ocr(job, image)
    }
}

#[derive(Debug, Default)]
struct Win32Clipboard;

impl Clipboard for Win32Clipboard {
    fn read(&self) -> Result<ClipboardPayload, PlatformError> {
        read_clipboard_payload()
    }

    fn write(&self, payload: ClipboardPayload) -> Result<(), PlatformError> {
        write_clipboard_payload(payload)
    }
}

#[derive(Debug, Default)]
struct Win32GlobalHotkey;

impl GlobalHotkey for Win32GlobalHotkey {
    fn register(
        &self,
        registration: HotkeyRegistration,
        sink: HotkeyEventSink,
    ) -> Result<Box<dyn HotkeyToken>, PlatformError> {
        let listener = listen_for_hotkey(registration, move |registration| {
            sink(registration);
        })?;
        Ok(Box::new(listener))
    }
}

#[derive(Debug, Default)]
struct Win32WindowOps;

impl WindowOps for Win32WindowOps {
    fn capture_window_regions(
        &self,
        screen_bounds: Rect,
    ) -> Result<Vec<platform_api::CaptureWindowRegion>, PlatformError> {
        Ok(crate::capture_window_regions(screen_bounds))
    }

    fn set_always_on_top(
        &self,
        window: PlatformWindowRef,
        enabled: bool,
    ) -> Result<(), PlatformError> {
        set_always_on_top(window.raw_handle(), enabled)
    }

    fn set_click_through(
        &self,
        window: PlatformWindowRef,
        enabled: bool,
    ) -> Result<(), PlatformError> {
        set_click_through(window.raw_handle(), enabled)
    }

    fn park_window(&self, window: PlatformWindowRef, bounds: Rect) -> Result<(), PlatformError> {
        crate::try_park_window(window.raw_handle(), bounds, true)
    }

    fn move_client_area_to(
        &self,
        window: PlatformWindowRef,
        position: shared_models::Point,
    ) -> Result<(), PlatformError> {
        crate::move_client_area_to(window.raw_handle(), position)
    }

    fn suspend_for_modal(&self, window: PlatformWindowRef) -> Result<(), PlatformError> {
        crate::try_suspend_window_for_modal_dialog(window.raw_handle())
    }

    fn restore_after_modal(
        &self,
        window: PlatformWindowRef,
        always_on_top: bool,
    ) -> Result<(), PlatformError> {
        crate::try_restore_window_after_modal_dialog(window.raw_handle(), always_on_top)
    }
}

#[derive(Debug, Default)]
struct Win32FileDialog;

impl FileDialog for Win32FileDialog {
    fn pick_folder(&self, title: &str) -> Result<Option<std::path::PathBuf>, PlatformError> {
        prompt_folder_path(title)
    }

    fn save_png_path(
        &self,
        default_name: &str,
    ) -> Result<Option<std::path::PathBuf>, PlatformError> {
        prompt_save_png_path(default_name)
    }
}

#[derive(Debug, Default)]
struct Win32SharedMemory;

impl SharedMemory for Win32SharedMemory {
    fn create(
        &self,
        request: SharedMemoryCreateRequest,
    ) -> Result<SharedMemoryHandle, PlatformError> {
        let byte_len = request.bytes.len();
        let mapping = create_named_shared_memory(&request.name, &request.bytes)?;
        Ok(SharedMemoryHandle::with_lease(
            request.name,
            byte_len,
            mapping,
        ))
    }

    fn open(&self, name: &str, byte_len: usize) -> Result<Vec<u8>, PlatformError> {
        read_named_shared_memory(name, byte_len)
    }
}

fn capture_with_backends(request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
    let backends: [&dyn WindowsCaptureBackend; 3] =
        [&WgcCaptureBackend, &DxgiCaptureBackend, &GdiCaptureBackend];
    let mut last_error = None;

    for backend in backends {
        if backend.kind() == CaptureBackendKind::Dxgi && dxgi_cooldown_active() {
            log::info!("skipping DXGI screen capture backend after recent recoverable failure");
            continue;
        }

        match backend.capture(request.clone()) {
            Ok(frame) => {
                if backend.kind() == CaptureBackendKind::Dxgi {
                    clear_dxgi_cooldown();
                }
                return Ok(frame);
            }
            Err(error) if error.code == "not_implemented" => {
                last_error = Some(error);
            }
            Err(error) => {
                log::warn!(
                    "screen capture backend failed kind={:?}: {error}",
                    backend.kind()
                );
                if backend.kind() == CaptureBackendKind::Dxgi && should_cool_down_dxgi(&error) {
                    cool_down_dxgi(&error);
                }
                last_error = Some(error);
            }
        }
    }

    last_error.map_or_else(|| capture_region(request.region), Err)
}

fn dxgi_cooldown_active() -> bool {
    let now = Instant::now();
    let Ok(mut skip_until) = DXGI_SKIP_UNTIL.lock() else {
        return false;
    };

    if let Some(until) = *skip_until {
        if now < until {
            return true;
        }
        *skip_until = None;
    }

    false
}

fn cool_down_dxgi(error: &PlatformError) {
    if let Ok(mut skip_until) = DXGI_SKIP_UNTIL.lock() {
        *skip_until = Some(Instant::now() + DXGI_FAILURE_COOLDOWN);
    }
    log::info!(
        "temporarily cooling down DXGI screen capture backend code={} cooldown_ms={}",
        error.code,
        DXGI_FAILURE_COOLDOWN.as_millis()
    );
}

fn clear_dxgi_cooldown() {
    if let Ok(mut skip_until) = DXGI_SKIP_UNTIL.lock() {
        *skip_until = None;
    }
}

fn should_cool_down_dxgi(error: &PlatformError) -> bool {
    matches!(
        error.code.as_str(),
        "dxgi_empty_frame" | "dxgi_frame_timeout" | "dxgi_capture_empty"
    )
}

#[cfg(windows)]
fn win32_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        screen_capture: CapabilityStatus::Supported,
        overlay_window: CapabilityStatus::Supported,
        pin_window: CapabilityStatus::Supported,
        system_ocr: system_ocr_status(),
        clipboard_read: CapabilityStatus::Supported,
        clipboard_write: CapabilityStatus::Supported,
        global_hotkey: CapabilityStatus::Supported,
        file_dialog: CapabilityStatus::Supported,
        shared_memory: CapabilityStatus::Supported,
        secure_storage: CapabilityStatus::unavailable(
            "secure_storage_not_implemented",
            "secure storage has not been implemented for Windows",
        ),
    }
}

#[cfg(not(windows))]
fn win32_capabilities() -> PlatformCapabilities {
    PlatformCapabilities::unavailable(
        "platform_implementation_target_mismatch",
        "Windows platform capabilities are available only on Windows",
    )
}

#[cfg(windows)]
fn system_ocr_status() -> CapabilityStatus {
    CapabilityStatus::Supported
}

#[cfg(not(windows))]
fn system_ocr_status() -> CapabilityStatus {
    CapabilityStatus::unavailable(
        "system_ocr_platform_only",
        "Windows system OCR is available only on Windows",
    )
}
