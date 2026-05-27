use shared_models::Rect;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureWindowRegion {
    pub bounds: Rect,
    pub depth: u8,
}

pub fn capture_window_regions(screen_bounds: Rect) -> Vec<CaptureWindowRegion> {
    platform::capture_window_regions(screen_bounds)
}

pub fn suspend_window_for_modal_dialog(hwnd: isize) {
    platform::suspend_window_for_modal_dialog(hwnd);
}

pub fn restore_window_after_modal_dialog(hwnd: isize, always_on_top: bool) {
    platform::restore_window_after_modal_dialog(hwnd, always_on_top);
}

pub fn park_window(hwnd: isize, bounds: Rect, always_on_top: bool) {
    platform::park_window(hwnd, bounds, always_on_top);
}

#[cfg(windows)]
mod platform {
    use shared_models::{Point, Rect, Size};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT as WinRect};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetWindowRect, HWND_NOTOPMOST, HWND_TOPMOST,
        IsWindowVisible, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        ShowWindow,
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
        let hwnd = hwnd as HWND;
        if hwnd.is_null() {
            return;
        }

        log::info!("suspending window for modal dialog hwnd={hwnd:?}");
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }

    pub fn restore_window_after_modal_dialog(hwnd: isize, always_on_top: bool) {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() {
            return;
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
            let _ = SetWindowPos(
                hwnd,
                insert_after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }

    pub fn park_window(hwnd: isize, bounds: Rect, always_on_top: bool) {
        let hwnd = hwnd as HWND;
        if hwnd.is_null() {
            return;
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
        unsafe {
            let _ = SetWindowPos(hwnd, insert_after, x, y, width, height, SWP_NOACTIVATE);
            let _ = ShowWindow(hwnd, SW_SHOWNA);
        }
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
}

#[cfg(not(windows))]
mod platform {
    use shared_models::Rect;

    use super::CaptureWindowRegion;

    pub fn capture_window_regions(_screen_bounds: Rect) -> Vec<CaptureWindowRegion> {
        Vec::new()
    }

    pub fn suspend_window_for_modal_dialog(_hwnd: isize) {}

    pub fn restore_window_after_modal_dialog(_hwnd: isize, _always_on_top: bool) {}

    pub fn park_window(_hwnd: isize, _bounds: Rect, _always_on_top: bool) {}
}
