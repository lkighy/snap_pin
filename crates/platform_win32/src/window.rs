pub use platform_api::{CaptureWindowRegion, NativeWindowRef, WindowOps};
use platform_api::{PlatformError, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub isize);

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayWindowOptions {
    pub bounds: Rect,
    pub transparent: bool,
    pub click_through: bool,
    pub always_on_top: bool,
}

impl OverlayWindowOptions {
    pub fn fullscreen(bounds: Rect) -> Self {
        Self {
            bounds,
            transparent: true,
            click_through: false,
            always_on_top: true,
        }
    }
}

pub fn capture_window_regions(screen_bounds: Rect) -> Vec<CaptureWindowRegion> {
    platform::capture_window_regions(screen_bounds)
}

pub fn suspend_window_for_modal_dialog(hwnd: isize) {
    platform::suspend_window_for_modal_dialog(hwnd);
}

pub fn try_suspend_window_for_modal_dialog(hwnd: isize) -> Result<(), PlatformError> {
    platform::try_suspend_window_for_modal_dialog(hwnd)
}

pub fn restore_window_after_modal_dialog(hwnd: isize, always_on_top: bool) {
    platform::restore_window_after_modal_dialog(hwnd, always_on_top);
}

pub fn try_restore_window_after_modal_dialog(
    hwnd: isize,
    always_on_top: bool,
) -> Result<(), PlatformError> {
    platform::try_restore_window_after_modal_dialog(hwnd, always_on_top)
}

pub fn park_window(hwnd: isize, bounds: Rect, always_on_top: bool) {
    platform::park_window(hwnd, bounds, always_on_top);
}

pub fn try_park_window(
    hwnd: isize,
    bounds: Rect,
    always_on_top: bool,
) -> Result<(), PlatformError> {
    platform::try_park_window(hwnd, bounds, always_on_top)
}

pub fn set_always_on_top(hwnd: isize, enabled: bool) -> Result<(), PlatformError> {
    platform::set_always_on_top(hwnd, enabled)
}

pub fn set_click_through(hwnd: isize, enabled: bool) -> Result<(), PlatformError> {
    platform::set_click_through(hwnd, enabled)
}

#[cfg(windows)]
mod platform {
    use platform_api::PlatformError;
    use shared_models::{Point, Rect, Size};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT as WinRect};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GWL_EXSTYLE, GetWindowLongW, GetWindowRect, HWND_NOTOPMOST,
        HWND_TOPMOST, IsWindow, IsWindowVisible, LWA_ALPHA, SW_HIDE, SW_SHOWNA, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetLayeredWindowAttributes, SetWindowLongW,
        SetWindowPos, ShowWindow, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    };

    use super::CaptureWindowRegion;

    const MAX_CAPTURE_REGIONS: usize = 1024;
    const MIN_CAPTURE_SIDE: f32 = 8.0;
    const MIN_VISIBLE_AREA: f32 = 64.0;

    pub fn capture_window_regions(screen_bounds: Rect) -> Vec<CaptureWindowRegion> {
        let mut context = EnumContext {
            screen_bounds,
            regions: Vec::new(),
            occluders: Vec::new(),
        };

        unsafe {
            let _ = EnumWindows(Some(enum_window_proc), &mut context as *mut _ as LPARAM);
        }

        context.regions
    }

    pub fn suspend_window_for_modal_dialog(hwnd: isize) {
        let _ = try_suspend_window_for_modal_dialog(hwnd);
    }

    pub fn try_suspend_window_for_modal_dialog(hwnd: isize) -> Result<(), PlatformError> {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() {
            return Err(invalid_window());
        }

        log::info!("suspending window for modal dialog hwnd={hwnd:?}");
        let positioned = unsafe {
            SetWindowPos(
                hwnd,
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
        if positioned == 0 {
            return Err(PlatformError::new(
                "window_topmost_failed",
                "failed to remove topmost state before modal dialog",
            ));
        }

        let shown = unsafe { ShowWindow(hwnd, SW_HIDE) };
        if shown == 0 {
            log::debug!("modal suspend hid a window that was already hidden");
        }
        Ok(())
    }

    pub fn restore_window_after_modal_dialog(hwnd: isize, always_on_top: bool) {
        let _ = try_restore_window_after_modal_dialog(hwnd, always_on_top);
    }

    pub fn try_restore_window_after_modal_dialog(
        hwnd: isize,
        always_on_top: bool,
    ) -> Result<(), PlatformError> {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() {
            return Err(invalid_window());
        }

        log::info!(
            "restoring window after modal dialog hwnd={hwnd:?} always_on_top={always_on_top}"
        );
        let insert_after = if always_on_top {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNA);
        }
        let positioned = unsafe {
            SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
        if positioned == 0 {
            return Err(PlatformError::new(
                "window_topmost_failed",
                "failed to restore window after modal dialog",
            ));
        }
        Ok(())
    }

    pub fn park_window(hwnd: isize, bounds: Rect, always_on_top: bool) {
        let _ = try_park_window(hwnd, bounds, always_on_top);
    }

    pub fn try_park_window(
        hwnd: isize,
        bounds: Rect,
        always_on_top: bool,
    ) -> Result<(), PlatformError> {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() {
            return Err(invalid_window());
        }

        let insert_after = if always_on_top {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        let x = bounds.origin.x.round() as i32;
        let y = bounds.origin.y.round() as i32;
        let width = bounds.size.width.round().max(1.0) as i32;
        let height = bounds.size.height.round().max(1.0) as i32;

        log::info!(
            "parking window hwnd={hwnd:?} x={x} y={y} width={width} height={height} always_on_top={always_on_top}"
        );
        let positioned =
            unsafe { SetWindowPos(hwnd, insert_after, x, y, width, height, SWP_NOACTIVATE) };
        if positioned == 0 {
            return Err(PlatformError::new(
                "window_park_failed",
                "failed to move the window to its parked bounds",
            ));
        }
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNA);
        }
        Ok(())
    }

    pub fn set_always_on_top(hwnd: isize, enabled: bool) -> Result<(), PlatformError> {
        let hwnd = validated_hwnd(hwnd)?;
        let insert_after = if enabled {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        let positioned = unsafe {
            SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
        if positioned == 0 {
            return Err(PlatformError::new(
                "window_topmost_failed",
                "failed to update the window topmost state",
            ));
        }
        Ok(())
    }

    pub fn set_click_through(hwnd: isize, enabled: bool) -> Result<(), PlatformError> {
        let hwnd = validated_hwnd(hwnd)?;
        let current = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
        let transparent = WS_EX_LAYERED | WS_EX_TRANSPARENT;
        let next = if enabled {
            current | transparent as i32
        } else {
            current & !(WS_EX_TRANSPARENT as i32)
        };
        let previous = unsafe { SetWindowLongW(hwnd, GWL_EXSTYLE, next) };
        if previous == 0 {
            return Err(PlatformError::new(
                "window_click_through_failed",
                "failed to update the window extended style",
            ));
        }

        if enabled {
            let alpha_set = unsafe { SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) };
            if alpha_set == 0 {
                return Err(PlatformError::new(
                    "window_click_through_failed",
                    "failed to preserve layered window alpha",
                ));
            }
        }

        let refreshed = unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            )
        };
        if refreshed == 0 {
            return Err(PlatformError::new(
                "window_click_through_failed",
                "failed to refresh the window style",
            ));
        }
        Ok(())
    }

    struct EnumContext {
        screen_bounds: Rect,
        regions: Vec<CaptureWindowRegion>,
        occluders: Vec<Rect>,
    }

    struct ChildEnumContext {
        parent: *mut EnumContext,
    }

    unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        let context = unsafe { &mut *(lparam as *mut EnumContext) };
        if context.regions.len() >= MAX_CAPTURE_REGIONS {
            return 0;
        }

        let Some(bounds) = candidate_window_bounds(context, hwnd) else {
            return 1;
        };

        if is_effectively_covered(bounds, &context.occluders) {
            return 1;
        }

        push_region(context, bounds, 0);

        let mut child_context = ChildEnumContext { parent: context };
        unsafe {
            let _ = EnumChildWindows(
                hwnd,
                Some(enum_child_window_proc),
                &mut child_context as *mut _ as LPARAM,
            );
        }

        context.occluders.push(bounds);
        1
    }

    unsafe extern "system" fn enum_child_window_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        let child_context = unsafe { &mut *(lparam as *mut ChildEnumContext) };
        let context = unsafe { &mut *child_context.parent };
        if context.regions.len() >= MAX_CAPTURE_REGIONS {
            return 0;
        }

        let Some(bounds) = candidate_window_bounds(context, hwnd) else {
            return 1;
        };

        if is_effectively_covered(bounds, &context.occluders) {
            return 1;
        }

        push_region(context, bounds, 1);
        1
    }

    fn candidate_window_bounds(context: &EnumContext, hwnd: HWND) -> Option<Rect> {
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return None;
        }

        let bounds = window_rect(hwnd).and_then(|rect| intersect(rect, context.screen_bounds))?;

        if bounds.size.width < MIN_CAPTURE_SIDE || bounds.size.height < MIN_CAPTURE_SIDE {
            return None;
        }

        Some(bounds)
    }

    fn push_region(context: &mut EnumContext, bounds: Rect, depth: u8) {
        if context
            .regions
            .iter()
            .any(|region| same_rect(region.bounds, bounds))
        {
            return;
        }

        context.regions.push(CaptureWindowRegion { bounds, depth });
    }

    fn window_rect(hwnd: HWND) -> Option<Rect> {
        let mut rect = WinRect::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
            return None;
        }

        let width = (rect.right - rect.left) as f32;
        let height = (rect.bottom - rect.top) as f32;
        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        Some(Rect::new(
            Point::new(rect.left as f32, rect.top as f32),
            Size::new(width, height),
        ))
    }

    fn intersect(a: Rect, b: Rect) -> Option<Rect> {
        let x1 = a.origin.x.max(b.origin.x);
        let y1 = a.origin.y.max(b.origin.y);
        let x2 = a.max_x().min(b.max_x());
        let y2 = a.max_y().min(b.max_y());

        (x2 > x1 && y2 > y1).then(|| Rect::new(Point::new(x1, y1), Size::new(x2 - x1, y2 - y1)))
    }

    fn is_effectively_covered(bounds: Rect, occluders: &[Rect]) -> bool {
        let mut visible_parts = vec![bounds];

        for occluder in occluders {
            let mut next_parts = Vec::new();
            for part in visible_parts {
                next_parts.extend(subtract_rect(part, *occluder));
            }

            visible_parts = next_parts;
            if visible_parts.iter().map(|part| part.area()).sum::<f32>() < MIN_VISIBLE_AREA {
                return true;
            }
        }

        false
    }

    fn subtract_rect(rect: Rect, cover: Rect) -> Vec<Rect> {
        let Some(overlap) = intersect(rect, cover) else {
            return vec![rect];
        };

        let mut parts = Vec::with_capacity(4);
        push_non_empty(
            &mut parts,
            Rect::new(
                rect.origin,
                Size::new(rect.size.width, overlap.origin.y - rect.origin.y),
            ),
        );
        push_non_empty(
            &mut parts,
            Rect::new(
                Point::new(rect.origin.x, overlap.max_y()),
                Size::new(rect.size.width, rect.max_y() - overlap.max_y()),
            ),
        );
        push_non_empty(
            &mut parts,
            Rect::new(
                Point::new(rect.origin.x, overlap.origin.y),
                Size::new(overlap.origin.x - rect.origin.x, overlap.size.height),
            ),
        );
        push_non_empty(
            &mut parts,
            Rect::new(
                Point::new(overlap.max_x(), overlap.origin.y),
                Size::new(rect.max_x() - overlap.max_x(), overlap.size.height),
            ),
        );

        parts
    }

    fn push_non_empty(parts: &mut Vec<Rect>, rect: Rect) {
        if rect.size.width > 0.5 && rect.size.height > 0.5 {
            parts.push(rect);
        }
    }

    fn same_rect(a: Rect, b: Rect) -> bool {
        a.origin.x.round() == b.origin.x.round()
            && a.origin.y.round() == b.origin.y.round()
            && a.size.width.round() == b.size.width.round()
            && a.size.height.round() == b.size.height.round()
    }

    fn validated_hwnd(hwnd: isize) -> Result<HWND, PlatformError> {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() || unsafe { IsWindow(hwnd) } == 0 {
            Err(invalid_window())
        } else {
            Ok(hwnd)
        }
    }

    fn invalid_window() -> PlatformError {
        PlatformError::new("invalid_window", "the native window handle is not valid")
    }
}

#[cfg(not(windows))]
mod platform {
    use platform_api::PlatformError;
    use shared_models::Rect;

    use super::CaptureWindowRegion;

    pub fn capture_window_regions(_screen_bounds: Rect) -> Vec<CaptureWindowRegion> {
        Vec::new()
    }

    pub fn suspend_window_for_modal_dialog(_hwnd: isize) {}

    pub fn try_suspend_window_for_modal_dialog(_hwnd: isize) -> Result<(), PlatformError> {
        Err(unsupported_window_ops())
    }

    pub fn restore_window_after_modal_dialog(_hwnd: isize, _always_on_top: bool) {}

    pub fn try_restore_window_after_modal_dialog(
        _hwnd: isize,
        _always_on_top: bool,
    ) -> Result<(), PlatformError> {
        Err(unsupported_window_ops())
    }

    pub fn park_window(_hwnd: isize, _bounds: Rect, _always_on_top: bool) {}

    pub fn try_park_window(
        _hwnd: isize,
        _bounds: Rect,
        _always_on_top: bool,
    ) -> Result<(), PlatformError> {
        Err(unsupported_window_ops())
    }

    pub fn set_always_on_top(_hwnd: isize, _enabled: bool) -> Result<(), PlatformError> {
        Err(unsupported_window_ops())
    }

    pub fn set_click_through(_hwnd: isize, _enabled: bool) -> Result<(), PlatformError> {
        Err(unsupported_window_ops())
    }

    fn unsupported_window_ops() -> PlatformError {
        PlatformError::new(
            "unsupported_platform",
            "window operations are currently implemented only on Windows",
        )
    }
}
