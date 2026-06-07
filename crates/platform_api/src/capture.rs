use shared_models::{ImageFormat, Rect, Size};

use crate::PlatformError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendHint {
    BestAvailable,
    LowLatency,
    Compatibility,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptureRequest {
    pub region: Option<Rect>,
    pub include_cursor: bool,
    pub backend_hint: Option<CaptureBackendHint>,
}

impl CaptureRequest {
    pub fn new(region: Option<Rect>) -> Self {
        Self {
            region,
            include_cursor: false,
            backend_hint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedFrame {
    pub pixel_size: Size,
    pub scale_factor: f32,
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    pub id: String,
    pub name: Option<String>,
    pub bounds: Rect,
    pub scale_factor: f32,
    pub primary: bool,
}

pub trait ScreenCapture: Send + Sync {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError>;
    fn virtual_bounds(&self) -> Result<Rect, PlatformError>;
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError>;
}
