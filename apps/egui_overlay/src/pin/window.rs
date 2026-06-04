use eframe::egui::{
    Color32, Context, CornerRadius, FontId, Painter, Pos2, Rect as EguiRect, Stroke, StrokeKind,
    Vec2,
};

pub(crate) const PIN_OPACITY_STEP: f32 = 0.05;
pub(crate) const PIN_MIN_OPACITY: f32 = 0.2;
pub(crate) const DEFAULT_PIN_MIN_WIDTH: f32 = 96.0;
pub(crate) const DEFAULT_PIN_MIN_HEIGHT: f32 = 72.0;
const PIN_MAX_SIDE: f32 = 8192.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PinWindowSizing {
    pub(crate) min_width: f32,
    pub(crate) min_height: f32,
}

impl Default for PinWindowSizing {
    fn default() -> Self {
        Self {
            min_width: DEFAULT_PIN_MIN_WIDTH,
            min_height: DEFAULT_PIN_MIN_HEIGHT,
        }
    }
}

impl PinWindowSizing {
    pub(crate) fn new(min_width: f32, min_height: f32) -> Self {
        Self {
            min_width: min_width.clamp(16.0, 2048.0),
            min_height: min_height.clamp(16.0, 2048.0),
        }
    }
}

pub(crate) fn clamp_pin_window_size(size: Vec2, sizing: PinWindowSizing) -> Vec2 {
    let mut size = size;
    if size.x < sizing.min_width {
        size *= sizing.min_width / size.x.max(1.0);
    }
    if size.y < sizing.min_height {
        size *= sizing.min_height / size.y.max(1.0);
    }
    if size.x > PIN_MAX_SIDE {
        size *= PIN_MAX_SIDE / size.x;
    }
    if size.y > PIN_MAX_SIDE {
        size *= PIN_MAX_SIDE / size.y;
    }
    size
}

pub(crate) fn fit_pin_image_size_to_canvas(
    image_size: Vec2,
    canvas_size: Vec2,
    sizing: PinWindowSizing,
) -> Vec2 {
    if image_size.x <= 0.0 || image_size.y <= 0.0 || canvas_size.x <= 0.0 || canvas_size.y <= 0.0 {
        return image_size.max(Vec2::splat(1.0));
    }

    let scale = (canvas_size.x / image_size.x).min(canvas_size.y / image_size.y);
    clamp_pin_window_size(image_size * scale, sizing)
}

pub(crate) fn current_viewport_rect(ctx: &Context) -> Option<EguiRect> {
    ctx.input(|input| input.viewport().inner_rect.or(input.viewport().outer_rect))
}

pub(crate) fn draw_pin_status(painter: &Painter, canvas: EguiRect, status: &str) {
    let galley = painter.layout_no_wrap(
        status.to_owned(),
        FontId::proportional(12.0),
        Color32::from_white_alpha(235),
    );
    let size = galley.size() + Vec2::new(14.0, 8.0);
    let rect = EguiRect::from_min_size(Pos2::new(canvas.min.x + 8.0, canvas.min.y + 8.0), size);
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(222));
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_white_alpha(34)),
        StrokeKind::Inside,
    );
    painter.galley(
        rect.min + Vec2::new(7.0, 4.0),
        galley,
        Color32::from_white_alpha(235),
    );
}
