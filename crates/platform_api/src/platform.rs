use shared_models::{ImageData, OcrJob, OcrResult};

use crate::{
    CapabilityStatus, Clipboard, FileDialog, GlobalHotkey, PlatformCapabilities, PlatformError,
    ScreenCapture, SharedMemory, WindowOps,
};

pub trait SystemOcr: Send + Sync {
    fn availability(&self) -> CapabilityStatus;
    fn recognize(&self, job: &OcrJob, image: &ImageData) -> Result<OcrResult, PlatformError>;
}

pub trait AppPlatform: Send + Sync {
    fn capabilities(&self) -> PlatformCapabilities;
    fn screen_capture(&self) -> &dyn ScreenCapture;
    fn system_ocr(&self) -> &dyn SystemOcr;
    fn clipboard(&self) -> &dyn Clipboard;
    fn global_hotkey(&self) -> &dyn GlobalHotkey;
    fn window_ops(&self) -> &dyn WindowOps;
    fn file_dialog(&self) -> &dyn FileDialog;
    fn shared_memory(&self) -> &dyn SharedMemory;
}
