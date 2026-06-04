use eframe::egui::{
    Color32, CornerRadius, FontId, Painter, Pos2, Rect as EguiRect, Stroke, StrokeKind, Vec2,
};

use crate::runtime::text::OverlayText;

const PIN_TOOLBAR_BUTTON_SIZE: f32 = 30.0;
const PIN_TOOLBAR_BUTTON_GAP: f32 = 6.0;
const PIN_TOOLBAR_PRIMARY_BUTTONS: usize = 5;
const PIN_TOOLBAR_OCR_EXTENSION_BUTTONS: usize = 3;
const PIN_TOOLBAR_PADDING: f32 = 6.0;
const PIN_TOOLBAR_MARGIN: f32 = 8.0;
const PIN_TOOLBAR_OUTSIDE_GAP: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinToolbarAction {
    CopyImage,
    CopySelectedText,
    CopyAllText,
    SaveImage,
    RunOcr,
    CloseOcr,
    Translate,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinToolbarEdge {
    Top,
    Right,
    Bottom,
    Left,
}

impl PinToolbarEdge {
    pub(crate) fn nearest(canvas: EguiRect, position: Pos2) -> Self {
        let left = (position.x - canvas.left()).abs();
        let right = (canvas.right() - position.x).abs();
        let top = (position.y - canvas.top()).abs();
        let bottom = (canvas.bottom() - position.y).abs();

        let mut edge = Self::Top;
        let mut distance = top;
        for (candidate, candidate_distance) in [
            (Self::Right, right),
            (Self::Bottom, bottom),
            (Self::Left, left),
        ] {
            if candidate_distance < distance {
                edge = candidate;
                distance = candidate_distance;
            }
        }

        edge
    }

    fn is_horizontal(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

#[derive(Debug, Clone, Copy)]
struct PinToolbarButton {
    rect: EguiRect,
    label: &'static str,
    shortcut: Option<&'static str>,
    action: PinToolbarAction,
    enabled: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PinToolbarState {
    pub(crate) text: OverlayText,
    pub(crate) ocr_active: bool,
    pub(crate) has_ocr_text: bool,
    pub(crate) has_selected_ocr_text: bool,
}

fn pin_toolbar_size(edge: PinToolbarEdge, button_count: usize) -> Vec2 {
    let count = button_count.max(1) as f32;
    let long_side = PIN_TOOLBAR_PADDING * 2.0
        + PIN_TOOLBAR_BUTTON_SIZE * count
        + PIN_TOOLBAR_BUTTON_GAP * (count - 1.0);
    let short_side = PIN_TOOLBAR_PADDING * 2.0 + PIN_TOOLBAR_BUTTON_SIZE;
    if edge.is_horizontal() {
        Vec2::new(long_side, short_side)
    } else {
        Vec2::new(short_side, long_side)
    }
}

pub(crate) fn pin_toolbar_extent(edge: PinToolbarEdge, state: PinToolbarState) -> f32 {
    let size = pin_toolbar_size(edge, PIN_TOOLBAR_PRIMARY_BUTTONS);
    let extension = if state.ocr_active {
        PIN_TOOLBAR_OUTSIDE_GAP + extension_short_side()
    } else {
        0.0
    };
    let toolbar_short_side = if edge.is_horizontal() { size.y } else { size.x };
    toolbar_short_side + PIN_TOOLBAR_OUTSIDE_GAP + extension + PIN_TOOLBAR_MARGIN
}

pub(crate) fn pin_window_size_for_image(
    image_size: Vec2,
    edge: Option<PinToolbarEdge>,
    state: PinToolbarState,
) -> Vec2 {
    let mut size = image_size;
    if let Some(edge) = edge {
        let extent = pin_toolbar_extent(edge, state);
        if edge.is_horizontal() {
            size.y += extent;
        } else {
            size.x += extent;
        }
    }

    size
}

pub(crate) fn pin_image_rect(
    canvas: EguiRect,
    image_size: Vec2,
    edge: Option<PinToolbarEdge>,
    state: PinToolbarState,
) -> EguiRect {
    let min = match edge {
        Some(PinToolbarEdge::Top) => Pos2::new(
            canvas.min.x,
            canvas.min.y + pin_toolbar_extent(PinToolbarEdge::Top, state),
        ),
        Some(PinToolbarEdge::Left) => Pos2::new(
            canvas.min.x + pin_toolbar_extent(PinToolbarEdge::Left, state),
            canvas.min.y,
        ),
        _ => canvas.min,
    };

    EguiRect::from_min_size(min, image_size)
}

pub(crate) fn pin_toolbar_rect(
    canvas: EguiRect,
    image_rect: EguiRect,
    edge: PinToolbarEdge,
    _state: PinToolbarState,
) -> EguiRect {
    let size = pin_toolbar_size(edge, PIN_TOOLBAR_PRIMARY_BUTTONS);
    let center = match edge {
        PinToolbarEdge::Top => Pos2::new(
            image_rect.center().x,
            image_rect.top() - PIN_TOOLBAR_OUTSIDE_GAP - size.y * 0.5,
        ),
        PinToolbarEdge::Right => Pos2::new(
            image_rect.right() + PIN_TOOLBAR_OUTSIDE_GAP + size.x * 0.5,
            image_rect.center().y,
        ),
        PinToolbarEdge::Bottom => Pos2::new(
            image_rect.center().x,
            image_rect.bottom() + PIN_TOOLBAR_OUTSIDE_GAP + size.y * 0.5,
        ),
        PinToolbarEdge::Left => Pos2::new(
            image_rect.left() - PIN_TOOLBAR_OUTSIDE_GAP - size.x * 0.5,
            image_rect.center().y,
        ),
    };
    let mut min = center - size * 0.5;
    let min_x = canvas.min.x + PIN_TOOLBAR_MARGIN;
    let min_y = canvas.min.y + PIN_TOOLBAR_MARGIN;
    let max_x = (canvas.max.x - size.x - PIN_TOOLBAR_MARGIN).max(min_x);
    let max_y = (canvas.max.y - size.y - PIN_TOOLBAR_MARGIN).max(min_y);
    min.x = min.x.clamp(min_x, max_x);
    min.y = min.y.clamp(min_y, max_y);

    EguiRect::from_min_size(min, size)
}

pub(crate) fn pin_toolbar_bounds(
    canvas: EguiRect,
    toolbar: EguiRect,
    state: PinToolbarState,
) -> EguiRect {
    pin_toolbar_extension_rect(canvas, toolbar, state)
        .map_or(toolbar, |extension| toolbar.union(extension))
}

fn extension_short_side() -> f32 {
    PIN_TOOLBAR_PADDING * 2.0 + PIN_TOOLBAR_BUTTON_SIZE
}

fn pin_toolbar_primary_buttons(toolbar: EguiRect, state: PinToolbarState) -> Vec<PinToolbarButton> {
    let horizontal = toolbar.width() >= toolbar.height();
    let first = toolbar.min + Vec2::splat(PIN_TOOLBAR_PADDING);
    let step = PIN_TOOLBAR_BUTTON_SIZE + PIN_TOOLBAR_BUTTON_GAP;
    let button_min = |index: f32| {
        if horizontal {
            Pos2::new(first.x + step * index, first.y)
        } else {
            Pos2::new(first.x, first.y + step * index)
        }
    };
    let text = state.text;
    vec![
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(0.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: text.copy_image_action,
            shortcut: Some("Ctrl+C"),
            action: PinToolbarAction::CopyImage,
            enabled: true,
        },
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(1.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: text.save_image_action,
            shortcut: None,
            action: PinToolbarAction::SaveImage,
            enabled: true,
        },
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(2.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: if state.ocr_active {
                text.close_ocr_action
            } else {
                text.ocr_action
            },
            shortcut: None,
            action: if state.ocr_active {
                PinToolbarAction::CloseOcr
            } else {
                PinToolbarAction::RunOcr
            },
            enabled: true,
        },
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(3.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: text.translate_action,
            shortcut: None,
            action: PinToolbarAction::Translate,
            enabled: true,
        },
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(4.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: text.close_action,
            shortcut: Some("Esc"),
            action: PinToolbarAction::Close,
            enabled: true,
        },
    ]
}

fn pin_toolbar_extension_rect(
    canvas: EguiRect,
    toolbar: EguiRect,
    state: PinToolbarState,
) -> Option<EguiRect> {
    if !state.ocr_active {
        return None;
    }

    let ocr_button = pin_toolbar_primary_buttons(toolbar, state)
        .into_iter()
        .find(|button| matches!(button.action, PinToolbarAction::CloseOcr))?;
    let horizontal = toolbar.width() >= toolbar.height();
    let size = if horizontal {
        pin_toolbar_size(PinToolbarEdge::Top, PIN_TOOLBAR_OCR_EXTENSION_BUTTONS)
    } else {
        pin_toolbar_size(PinToolbarEdge::Left, PIN_TOOLBAR_OCR_EXTENSION_BUTTONS)
    };

    let mut min = if horizontal {
        let y = if toolbar.center().y < canvas.center().y {
            toolbar.min.y - PIN_TOOLBAR_OUTSIDE_GAP - size.y
        } else {
            toolbar.max.y + PIN_TOOLBAR_OUTSIDE_GAP
        };
        Pos2::new(ocr_button.rect.center().x - size.x * 0.5, y)
    } else {
        let x = if toolbar.center().x < canvas.center().x {
            toolbar.min.x - PIN_TOOLBAR_OUTSIDE_GAP - size.x
        } else {
            toolbar.max.x + PIN_TOOLBAR_OUTSIDE_GAP
        };
        Pos2::new(x, ocr_button.rect.center().y - size.y * 0.5)
    };

    min.x = min.x.clamp(
        canvas.min.x + PIN_TOOLBAR_MARGIN,
        (canvas.max.x - size.x - PIN_TOOLBAR_MARGIN).max(canvas.min.x + PIN_TOOLBAR_MARGIN),
    );
    min.y = min.y.clamp(
        canvas.min.y + PIN_TOOLBAR_MARGIN,
        (canvas.max.y - size.y - PIN_TOOLBAR_MARGIN).max(canvas.min.y + PIN_TOOLBAR_MARGIN),
    );

    Some(EguiRect::from_min_size(min, size))
}

fn pin_toolbar_extension_buttons(
    canvas: EguiRect,
    toolbar: EguiRect,
    state: PinToolbarState,
) -> Vec<PinToolbarButton> {
    let Some(extension) = pin_toolbar_extension_rect(canvas, toolbar, state) else {
        return Vec::new();
    };

    let horizontal = extension.width() >= extension.height();
    let first = extension.min + Vec2::splat(PIN_TOOLBAR_PADDING);
    let step = PIN_TOOLBAR_BUTTON_SIZE + PIN_TOOLBAR_BUTTON_GAP;
    let button_min = |index: f32| {
        if horizontal {
            Pos2::new(first.x + step * index, first.y)
        } else {
            Pos2::new(first.x, first.y + step * index)
        }
    };

    vec![
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(0.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: state.text.copy_selected_text_action,
            shortcut: Some("Ctrl+C"),
            action: PinToolbarAction::CopySelectedText,
            enabled: state.has_selected_ocr_text,
        },
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(1.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: state.text.copy_all_text_action,
            shortcut: Some("Ctrl+Shift+C"),
            action: PinToolbarAction::CopyAllText,
            enabled: state.has_ocr_text,
        },
        PinToolbarButton {
            rect: EguiRect::from_min_size(button_min(2.0), Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE)),
            label: state.text.rerun_ocr_action,
            shortcut: None,
            action: PinToolbarAction::RunOcr,
            enabled: true,
        },
    ]
}

fn pin_toolbar_buttons(
    canvas: EguiRect,
    toolbar: EguiRect,
    state: PinToolbarState,
) -> Vec<PinToolbarButton> {
    let mut buttons = pin_toolbar_primary_buttons(toolbar, state);
    buttons.extend(pin_toolbar_extension_buttons(canvas, toolbar, state));
    buttons
}

pub(crate) fn pin_toolbar_action_at(
    position: Pos2,
    canvas: EguiRect,
    toolbar: EguiRect,
    state: PinToolbarState,
) -> Option<PinToolbarAction> {
    pin_toolbar_buttons(canvas, toolbar, state)
        .into_iter()
        .find(|button| button.enabled && button.rect.contains(position))
        .map(|button| button.action)
}

pub(crate) fn draw_pin_toolbar(
    painter: &Painter,
    canvas: EguiRect,
    toolbar: EguiRect,
    state: PinToolbarState,
) {
    let pointer = painter.ctx().input(|input| input.pointer.hover_pos());
    painter.rect_filled(toolbar, 0.0, Color32::from_black_alpha(222));
    painter.rect_stroke(
        toolbar,
        CornerRadius::same(0),
        Stroke::new(1.0, Color32::from_white_alpha(40)),
        StrokeKind::Outside,
    );

    if let Some(extension) = pin_toolbar_extension_rect(canvas, toolbar, state) {
        painter.rect_filled(extension, 0.0, Color32::from_black_alpha(222));
        painter.rect_stroke(
            extension,
            CornerRadius::same(0),
            Stroke::new(1.0, Color32::from_white_alpha(40)),
            StrokeKind::Outside,
        );
    }

    for button in pin_toolbar_buttons(canvas, toolbar, state) {
        let hovered = pointer.is_some_and(|position| button.rect.contains(position));
        let fill = if hovered && button.enabled {
            Color32::from_white_alpha(36)
        } else {
            Color32::from_white_alpha(18)
        };
        painter.rect_filled(button.rect, 0.0, fill);
        painter.rect_stroke(
            button.rect,
            CornerRadius::same(0),
            Stroke::new(1.0, Color32::from_white_alpha(42)),
            StrokeKind::Inside,
        );
        let color = if button.enabled {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(92)
        };
        draw_pin_toolbar_icon(painter, button.rect, button.action, color);

        if hovered {
            draw_pin_toolbar_tooltip(
                painter,
                canvas,
                button.rect,
                button.label,
                button.shortcut,
                button.enabled,
            );
        }
    }
}

fn draw_pin_toolbar_icon(
    painter: &Painter,
    rect: EguiRect,
    action: PinToolbarAction,
    color: Color32,
) {
    match action {
        PinToolbarAction::CopyImage => draw_copy_image_icon(painter, rect, color),
        PinToolbarAction::CopySelectedText => draw_copy_text_icon(painter, rect, color),
        PinToolbarAction::CopyAllText => draw_copy_all_text_icon(painter, rect, color),
        PinToolbarAction::SaveImage => draw_save_icon(painter, rect, color),
        PinToolbarAction::RunOcr => draw_ocr_icon(painter, rect, color),
        PinToolbarAction::CloseOcr => draw_close_ocr_icon(painter, rect, color),
        PinToolbarAction::Translate => draw_translate_icon(painter, rect, color),
        PinToolbarAction::Close => draw_close_icon(painter, rect, color),
    }
}

fn draw_copy_image_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let back =
        EguiRect::from_center_size(rect.center() + Vec2::new(-3.0, -3.0), Vec2::new(13.0, 11.0));
    let front =
        EguiRect::from_center_size(rect.center() + Vec2::new(3.0, 3.0), Vec2::new(13.0, 11.0));
    painter.rect_stroke(back, CornerRadius::same(1), stroke, StrokeKind::Inside);
    painter.rect_filled(front, CornerRadius::same(1), Color32::from_black_alpha(20));
    painter.rect_stroke(front, CornerRadius::same(1), stroke, StrokeKind::Inside);
    painter.line_segment(
        [
            Pos2::new(front.min.x + 2.0, front.max.y - 3.0),
            Pos2::new(front.min.x + 5.0, front.center().y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(front.min.x + 5.0, front.center().y),
            Pos2::new(front.max.x - 2.0, front.max.y - 3.0),
        ],
        stroke,
    );
}

fn draw_copy_text_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let page = EguiRect::from_center_size(rect.center(), Vec2::new(16.0, 17.0));
    painter.rect_stroke(page, CornerRadius::same(1), stroke, StrokeKind::Inside);
    for offset in [5.0, 9.0, 13.0] {
        painter.line_segment(
            [
                Pos2::new(page.min.x + 3.0, page.min.y + offset),
                Pos2::new(page.max.x - 3.0, page.min.y + offset),
            ],
            stroke,
        );
    }
}

fn draw_copy_all_text_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    draw_copy_text_icon(painter, rect, color);
    let stroke = Stroke::new(1.4, color);
    let center = rect.center() + Vec2::new(6.0, -7.0);
    painter.line_segment(
        [center + Vec2::new(-3.0, 0.0), center + Vec2::new(3.0, 0.0)],
        stroke,
    );
    painter.line_segment(
        [center + Vec2::new(0.0, -3.0), center + Vec2::new(0.0, 3.0)],
        stroke,
    );
}

fn draw_save_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let body = EguiRect::from_center_size(rect.center(), Vec2::new(17.0, 17.0));
    painter.rect_stroke(body, CornerRadius::same(1), stroke, StrokeKind::Inside);
    painter.line_segment(
        [
            Pos2::new(body.min.x + 4.0, body.min.y + 4.0),
            Pos2::new(body.max.x - 4.0, body.min.y + 4.0),
        ],
        stroke,
    );
    let tray = EguiRect::from_min_max(
        Pos2::new(body.min.x + 4.0, body.max.y - 7.0),
        Pos2::new(body.max.x - 4.0, body.max.y - 3.0),
    );
    painter.rect_stroke(tray, CornerRadius::same(1), stroke, StrokeKind::Inside);
}

fn draw_ocr_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let box_rect = EguiRect::from_center_size(rect.center(), Vec2::new(17.0, 14.0));
    painter.rect_stroke(box_rect, CornerRadius::same(1), stroke, StrokeKind::Inside);
    painter.line_segment(
        [
            Pos2::new(box_rect.min.x + 3.0, box_rect.center().y),
            Pos2::new(box_rect.max.x - 3.0, box_rect.center().y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(box_rect.min.x + 3.0, box_rect.center().y + 4.0),
            Pos2::new(box_rect.max.x - 6.0, box_rect.center().y + 4.0),
        ],
        stroke,
    );
}

fn draw_close_ocr_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    draw_ocr_icon(painter, rect, color);
    let stroke = Stroke::new(1.7, color);
    let center = rect.center() + Vec2::new(5.5, -5.5);
    painter.line_segment(
        [center + Vec2::new(-3.0, -3.0), center + Vec2::new(3.0, 3.0)],
        stroke,
    );
    painter.line_segment(
        [center + Vec2::new(3.0, -3.0), center + Vec2::new(-3.0, 3.0)],
        stroke,
    );
}

fn draw_translate_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let center = rect.center();
    painter.line_segment(
        [
            center + Vec2::new(-8.0, -5.0),
            center + Vec2::new(2.0, -5.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            center + Vec2::new(-3.0, -9.0),
            center + Vec2::new(-3.0, 3.0),
        ],
        stroke,
    );
    painter.line_segment(
        [center + Vec2::new(3.0, 7.0), center + Vec2::new(9.0, 7.0)],
        stroke,
    );
    painter.line_segment(
        [center + Vec2::new(6.0, 1.0), center + Vec2::new(10.0, 11.0)],
        stroke,
    );
    painter.line_segment(
        [center + Vec2::new(6.0, 1.0), center + Vec2::new(2.0, 11.0)],
        stroke,
    );
    painter.line_segment(
        [center + Vec2::new(-8.0, 8.0), center + Vec2::new(9.0, -8.0)],
        stroke,
    );
}

fn draw_close_icon(painter: &Painter, rect: EguiRect, color: Color32) {
    let stroke = Stroke::new(1.8, color);
    let center = rect.center();
    painter.line_segment(
        [center + Vec2::new(-6.0, -6.0), center + Vec2::new(6.0, 6.0)],
        stroke,
    );
    painter.line_segment(
        [center + Vec2::new(6.0, -6.0), center + Vec2::new(-6.0, 6.0)],
        stroke,
    );
}

fn draw_pin_toolbar_tooltip(
    painter: &Painter,
    canvas: EguiRect,
    button: EguiRect,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) {
    let label = match shortcut {
        Some(shortcut) => format!("{label}  {shortcut}"),
        None => label.to_owned(),
    };
    let galley = painter.layout_no_wrap(
        label,
        FontId::proportional(12.0),
        if enabled {
            Color32::from_white_alpha(235)
        } else {
            Color32::from_white_alpha(150)
        },
    );
    let size = galley.size() + Vec2::new(14.0, 8.0);
    let mut min = Pos2::new(
        button.center().x - size.x * 0.5,
        button.min.y - size.y - 6.0,
    );
    if min.y < canvas.min.y + 6.0 {
        min.y = button.max.y + 6.0;
    }
    min.x = min.x.clamp(canvas.min.x + 6.0, canvas.max.x - size.x - 6.0);
    let rect = EguiRect::from_min_size(min, size);
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(230));
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_white_alpha(34)),
        StrokeKind::Inside,
    );
    painter.galley(
        Pos2::new(rect.min.x + 7.0, rect.min.y + 4.0),
        galley,
        Color32::from_white_alpha(235),
    );
}
