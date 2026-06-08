use shared_models::{Point, Rect};

use crate::PlatformError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeWindowRef {
    pub raw: isize,
}

impl NativeWindowRef {
    pub fn from_raw(raw: isize) -> Self {
        Self { raw }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureWindowRegion {
    pub bounds: Rect,
    pub depth: u8,
}

pub trait WindowOps: Send + Sync {
    fn capture_window_regions(
        &self,
        screen_bounds: Rect,
    ) -> Result<Vec<CaptureWindowRegion>, PlatformError>;

    fn set_always_on_top(
        &self,
        window: NativeWindowRef,
        enabled: bool,
    ) -> Result<(), PlatformError>;

    fn set_click_through(
        &self,
        window: NativeWindowRef,
        enabled: bool,
    ) -> Result<(), PlatformError>;

    fn park_window(&self, window: NativeWindowRef, bounds: Rect) -> Result<(), PlatformError>;

    fn move_client_area_to(
        &self,
        window: NativeWindowRef,
        position: Point,
    ) -> Result<(), PlatformError>;

    fn suspend_for_modal(&self, window: NativeWindowRef) -> Result<(), PlatformError>;

    fn restore_after_modal(
        &self,
        window: NativeWindowRef,
        always_on_top: bool,
    ) -> Result<(), PlatformError>;
}
