use platform_api::{
    AppPlatform, CapabilityStatus, Clipboard, ClipboardPayload, FileDialog, GlobalHotkey,
    HotkeyEventSink, HotkeyRegistration, HotkeyToken, ImageData, MonitorInfo, NativeWindowRef,
    OcrJob, OcrResult, PlatformCapabilities, PlatformCapability, PlatformError, Rect,
    ScreenCapture, SharedMemory, SharedMemoryCreateRequest, SharedMemoryHandle, SystemOcr,
    WindowOps,
};

#[derive(Debug)]
pub struct StubPlatform {
    capabilities: PlatformCapabilities,
    screen_capture: StubScreenCapture,
    system_ocr: StubSystemOcr,
    clipboard: StubClipboard,
    global_hotkey: StubGlobalHotkey,
    window_ops: StubWindowOps,
    file_dialog: StubFileDialog,
    shared_memory: StubSharedMemory,
}

impl Default for StubPlatform {
    fn default() -> Self {
        let reason = format!(
            "{} platform support has not been implemented yet",
            current_platform_name()
        );
        Self {
            capabilities: PlatformCapabilities::unavailable(reason.clone()),
            screen_capture: StubScreenCapture::new(reason.clone()),
            system_ocr: StubSystemOcr::new(reason.clone()),
            clipboard: StubClipboard::new(reason.clone()),
            global_hotkey: StubGlobalHotkey::new(reason.clone()),
            window_ops: StubWindowOps::new(reason.clone()),
            file_dialog: StubFileDialog::new(reason.clone()),
            shared_memory: StubSharedMemory::new(reason),
        }
    }
}

impl AppPlatform for StubPlatform {
    fn capabilities(&self) -> PlatformCapabilities {
        self.capabilities.clone()
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

#[derive(Debug)]
struct StubScreenCapture {
    reason: String,
}

impl StubScreenCapture {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl ScreenCapture for StubScreenCapture {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        Err(unavailable(
            PlatformCapability::ScreenCapture,
            self.reason.clone(),
        ))
    }

    fn virtual_bounds(&self) -> Result<Rect, PlatformError> {
        Err(unavailable(
            PlatformCapability::ScreenCapture,
            self.reason.clone(),
        ))
    }

    fn capture(
        &self,
        _request: platform_api::CaptureRequest,
    ) -> Result<platform_api::CapturedFrame, PlatformError> {
        Err(unavailable(
            PlatformCapability::ScreenCapture,
            self.reason.clone(),
        ))
    }
}

#[derive(Debug)]
struct StubSystemOcr {
    reason: String,
}

impl StubSystemOcr {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl SystemOcr for StubSystemOcr {
    fn availability(&self) -> CapabilityStatus {
        CapabilityStatus::unavailable(self.reason.clone())
    }

    fn recognize(&self, _job: &OcrJob, _image: &ImageData) -> Result<OcrResult, PlatformError> {
        Err(unavailable(
            PlatformCapability::SystemOcr,
            self.reason.clone(),
        ))
    }
}

#[derive(Debug)]
struct StubClipboard {
    reason: String,
}

impl StubClipboard {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl Clipboard for StubClipboard {
    fn read(&self) -> Result<ClipboardPayload, PlatformError> {
        Err(unavailable(
            PlatformCapability::ClipboardRead,
            self.reason.clone(),
        ))
    }

    fn write(&self, _payload: ClipboardPayload) -> Result<(), PlatformError> {
        Err(unavailable(
            PlatformCapability::ClipboardWrite,
            self.reason.clone(),
        ))
    }
}

#[derive(Debug)]
struct StubGlobalHotkey {
    reason: String,
}

impl StubGlobalHotkey {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl GlobalHotkey for StubGlobalHotkey {
    fn register(
        &self,
        _registration: HotkeyRegistration,
        _sink: HotkeyEventSink,
    ) -> Result<Box<dyn HotkeyToken>, PlatformError> {
        Err(unavailable(
            PlatformCapability::GlobalHotkey,
            self.reason.clone(),
        ))
    }
}

#[derive(Debug)]
struct StubWindowOps {
    reason: String,
}

impl StubWindowOps {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl WindowOps for StubWindowOps {
    fn capture_window_regions(
        &self,
        _screen_bounds: Rect,
    ) -> Result<Vec<platform_api::CaptureWindowRegion>, PlatformError> {
        Err(unavailable(
            PlatformCapability::OverlayWindow,
            self.reason.clone(),
        ))
    }

    fn set_always_on_top(
        &self,
        _window: NativeWindowRef,
        _enabled: bool,
    ) -> Result<(), PlatformError> {
        Err(unavailable(
            PlatformCapability::PinWindow,
            self.reason.clone(),
        ))
    }

    fn set_click_through(
        &self,
        _window: NativeWindowRef,
        _enabled: bool,
    ) -> Result<(), PlatformError> {
        Err(unavailable(
            PlatformCapability::PinWindow,
            self.reason.clone(),
        ))
    }

    fn park_window(&self, _window: NativeWindowRef, _bounds: Rect) -> Result<(), PlatformError> {
        Err(unavailable(
            PlatformCapability::OverlayWindow,
            self.reason.clone(),
        ))
    }

    fn move_client_area_to(
        &self,
        _window: NativeWindowRef,
        _position: shared_models::Point,
    ) -> Result<(), PlatformError> {
        Err(unavailable(
            PlatformCapability::PinWindow,
            self.reason.clone(),
        ))
    }

    fn suspend_for_modal(&self, _window: NativeWindowRef) -> Result<(), PlatformError> {
        Err(unavailable(
            PlatformCapability::OverlayWindow,
            self.reason.clone(),
        ))
    }

    fn restore_after_modal(
        &self,
        _window: NativeWindowRef,
        _always_on_top: bool,
    ) -> Result<(), PlatformError> {
        Err(unavailable(
            PlatformCapability::OverlayWindow,
            self.reason.clone(),
        ))
    }
}

#[derive(Debug)]
struct StubFileDialog {
    reason: String,
}

impl StubFileDialog {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl FileDialog for StubFileDialog {
    fn pick_folder(&self, _title: &str) -> Result<Option<std::path::PathBuf>, PlatformError> {
        Err(unavailable(
            PlatformCapability::FileDialog,
            self.reason.clone(),
        ))
    }

    fn save_png_path(
        &self,
        _default_name: &str,
    ) -> Result<Option<std::path::PathBuf>, PlatformError> {
        Err(unavailable(
            PlatformCapability::FileDialog,
            self.reason.clone(),
        ))
    }
}

#[derive(Debug)]
struct StubSharedMemory {
    reason: String,
}

impl StubSharedMemory {
    fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl SharedMemory for StubSharedMemory {
    fn create(
        &self,
        _request: SharedMemoryCreateRequest,
    ) -> Result<SharedMemoryHandle, PlatformError> {
        Err(unavailable(
            PlatformCapability::SharedMemory,
            self.reason.clone(),
        ))
    }

    fn open(&self, _name: &str, _byte_len: usize) -> Result<Vec<u8>, PlatformError> {
        Err(unavailable(
            PlatformCapability::SharedMemory,
            self.reason.clone(),
        ))
    }
}

fn unavailable(capability: PlatformCapability, reason: String) -> PlatformError {
    PlatformError::new("unsupported_platform", reason)
        .with_capability(capability)
        .with_recoverable(false)
}

fn current_platform_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "This"
    }
}
