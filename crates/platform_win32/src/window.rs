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
