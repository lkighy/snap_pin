use shared_models::{Point, Rect};

use crate::PlatformError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformWindowRef {
    raw_handle: isize,
}

impl PlatformWindowRef {
    pub fn from_raw_handle(raw_handle: isize) -> Self {
        Self { raw_handle }
    }

    pub fn raw_handle(self) -> isize {
        self.raw_handle
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
        window: PlatformWindowRef,
        enabled: bool,
    ) -> Result<(), PlatformError>;

    fn set_click_through(
        &self,
        window: PlatformWindowRef,
        enabled: bool,
    ) -> Result<(), PlatformError>;

    fn park_window(&self, window: PlatformWindowRef, bounds: Rect) -> Result<(), PlatformError>;

    fn move_client_area_to(
        &self,
        window: PlatformWindowRef,
        position: Point,
    ) -> Result<(), PlatformError>;

    fn suspend_for_modal(&self, window: PlatformWindowRef) -> Result<(), PlatformError>;

    fn restore_after_modal(
        &self,
        window: PlatformWindowRef,
        always_on_top: bool,
    ) -> Result<(), PlatformError>;
}
