pub use platform_api::{
    CaptureBackendHint, CaptureRequest, CapturedFrame, MonitorInfo, ScreenCapture,
};
use platform_api::{PlatformError, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    Dxgi,
    Gdi,
    Wgc,
}

pub trait WindowsCaptureBackend {
    fn kind(&self) -> CaptureBackendKind;
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WgcCaptureBackend;

impl WindowsCaptureBackend for WgcCaptureBackend {
    fn kind(&self) -> CaptureBackendKind {
        CaptureBackendKind::Wgc
    }

    fn capture(&self, _request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        Err(PlatformError::new(
            "not_implemented",
            "WGC capture backend is reserved for the Windows implementation phase",
        ))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DxgiCaptureBackend;

impl WindowsCaptureBackend for DxgiCaptureBackend {
    fn kind(&self) -> CaptureBackendKind {
        CaptureBackendKind::Dxgi
    }

    #[cfg(windows)]
    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        win32_dxgi::capture_region(request.region)
    }

    #[cfg(not(windows))]
    fn capture(&self, _request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        Err(PlatformError::new(
            "unsupported_platform",
            "DXGI capture backend is available only on Windows",
        ))
    }
}

#[cfg(windows)]
pub fn virtual_screen_bounds() -> Rect {
    win32_gdi::virtual_screen_bounds()
}

#[cfg(not(windows))]
pub fn virtual_screen_bounds() -> Rect {
    Rect::new(
        shared_models::Point::ZERO,
        shared_models::Size::new(1280.0, 720.0),
    )
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GdiCaptureBackend;

impl WindowsCaptureBackend for GdiCaptureBackend {
    fn kind(&self) -> CaptureBackendKind {
        CaptureBackendKind::Gdi
    }

    fn capture(&self, request: CaptureRequest) -> Result<CapturedFrame, PlatformError> {
        capture_region(request.region)
    }
}

#[cfg(windows)]
pub fn capture_region(region: Option<Rect>) -> Result<CapturedFrame, PlatformError> {
    win32_gdi::capture_region(region)
}

#[cfg(not(windows))]
pub fn capture_region(_region: Option<Rect>) -> Result<CapturedFrame, PlatformError> {
    Err(PlatformError::new(
        "unsupported_platform",
        "screen capture is currently implemented only on Windows",
    ))
}

// Keep the platform FFI implementations out of the public capture API surface.
#[cfg(windows)]
#[path = "capture/win32_gdi.rs"]
mod win32_gdi;

#[cfg(windows)]
#[path = "capture/win32_dxgi.rs"]
mod win32_dxgi;
