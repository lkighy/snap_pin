use std::borrow::Cow;

use eframe::egui::{
    Color32, CornerRadius, FontId, Painter, Pos2, Rect as EguiRect, Stroke, StrokeKind, Vec2,
};
use shared_models::{Rect, Size, TextOverlay};

const TEXT_MEASURE_WRAP_WIDTH: f32 = 1_000_000.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct OcrTextOverlayStyle {
    pub(crate) font_height_ratio: f32,
    pub(crate) min_font_size: f32,
    pub(crate) max_font_size: f32,
    pub(crate) padding_x: f32,
    pub(crate) padding_y: f32,
    pub(crate) interaction_padding_x: f32,
    pub(crate) interaction_padding_y: f32,
}

impl Default for OcrTextOverlayStyle {
    fn default() -> Self {
        Self {
            font_height_ratio: 0.46,
            min_font_size: 6.0,
            max_font_size: 42.0,
            padding_x: 2.0,
            padding_y: 1.0,
            interaction_padding_x: 2.0,
            interaction_padding_y: 4.0,
        }
    }
}

impl OcrTextOverlayStyle {
    pub(crate) fn new(
        font_height_ratio: f32,
        min_font_size: f32,
        max_font_size: f32,
        padding_x: f32,
        padding_y: f32,
        interaction_padding_x: f32,
        interaction_padding_y: f32,
    ) -> Self {
        let min_font_size = min_font_size.clamp(4.0, 96.0);
        Self {
            font_height_ratio: font_height_ratio.clamp(0.1, 2.0),
            min_font_size,
            max_font_size: max_font_size.clamp(min_font_size, 128.0),
            padding_x: padding_x.clamp(0.0, 32.0),
            padding_y: padding_y.clamp(0.0, 32.0),
            interaction_padding_x: interaction_padding_x.clamp(0.0, 48.0),
            interaction_padding_y: interaction_padding_y.clamp(0.0, 48.0),
        }
    }
}

pub(crate) fn draw_text_overlay(
    painter: &Painter,
    image_rect: EguiRect,
    image_size: Size,
    overlay: &TextOverlay,
    style: OcrTextOverlayStyle,
    selected: bool,
    right_limit_x: f32,
) {
    let text = text_overlay_display_text(&overlay.text);
    if text.is_empty() {
        return;
    }

    let bounds_rect = text_overlay_bounds_rect(image_rect, image_size, overlay);
    let font_size = text_overlay_font_size(bounds_rect, style);
    let label_rect = text_overlay_label_rect(
        painter,
        image_rect,
        bounds_rect,
        &text,
        font_size,
        style,
        right_limit_x,
    );
    let padding = Vec2::new(style.padding_x, style.padding_y);
    let galley = painter.layout(
        text.to_string(),
        FontId::proportional(font_size),
        Color32::WHITE,
        (label_rect.width() - padding.x * 2.0).max(1.0),
    );

    let fill = if selected {
        Color32::from_rgba_premultiplied(33, 118, 255, 214)
    } else {
        Color32::from_black_alpha(196)
    };
    let stroke = if selected {
        Stroke::new(1.5, Color32::from_rgb(165, 210, 255))
    } else {
        Stroke::new(1.0, Color32::from_white_alpha(56))
    };

    painter.rect_filled(label_rect, 0.0, fill);
    painter.rect_stroke(label_rect, CornerRadius::ZERO, stroke, StrokeKind::Inside);
    painter.galley(label_rect.min + padding, galley, Color32::WHITE);
}

pub(crate) fn ocr_block_interaction_rect(
    image_rect: EguiRect,
    image_size: Size,
    bounds: Rect,
    style: OcrTextOverlayStyle,
) -> EguiRect {
    image_bounds_to_screen(image_rect, image_size, bounds)
        .expand2(Vec2::new(
            style.interaction_padding_x,
            style.interaction_padding_y,
        ))
        .intersect(image_rect)
}

fn text_overlay_label_rect(
    painter: &Painter,
    image_rect: EguiRect,
    bounds_rect: EguiRect,
    text: &str,
    font_size: f32,
    style: OcrTextOverlayStyle,
    right_limit_x: f32,
) -> EguiRect {
    let padding = Vec2::new(style.padding_x, style.padding_y);
    let mut label_min = bounds_rect.left_top();
    label_min.x = label_min.x.clamp(
        image_rect.min.x,
        (image_rect.max.x - 1.0).max(image_rect.min.x),
    );

    let right_limit_x = if right_limit_x.is_finite() {
        right_limit_x
    } else {
        image_rect.max.x
    };
    let right_limit_x = right_limit_x.min(image_rect.max.x).max(label_min.x + 1.0);
    let available_width = (right_limit_x - label_min.x).max(1.0);
    let unwrapped_galley = painter.layout(
        text.to_owned(),
        FontId::proportional(font_size),
        Color32::WHITE,
        TEXT_MEASURE_WRAP_WIDTH,
    );
    let desired_width = unwrapped_galley.size().x + padding.x * 2.0;
    let label_width = desired_width.min(available_width).max(1.0);
    let content_width = (label_width - padding.x * 2.0).max(1.0);
    let galley = painter.layout(
        text.to_owned(),
        FontId::proportional(font_size),
        Color32::WHITE,
        content_width,
    );
    let label_size = Vec2::new(label_width, galley.size().y + padding.y * 2.0);
    label_min.y = label_min.y.clamp(
        image_rect.min.y,
        (image_rect.max.y - label_size.y).max(image_rect.min.y),
    );
    EguiRect::from_min_size(label_min, label_size)
}

fn text_overlay_display_text(text: &str) -> Cow<'_, str> {
    let text = text.trim();
    if text.is_empty() {
        return Cow::Borrowed(text);
    }

    let mut display_text = String::with_capacity(text.len());
    let mut pending_whitespace = String::new();
    let mut previous_compact = false;
    let mut changed = false;

    for character in text.chars() {
        if character.is_whitespace() {
            pending_whitespace.push(character);
            continue;
        }

        let current_compact = is_compact_ocr_text_character(character);
        if !pending_whitespace.is_empty() {
            if previous_compact && current_compact {
                changed = true;
            } else {
                display_text.push_str(&pending_whitespace);
            }
            pending_whitespace.clear();
        }

        display_text.push(character);
        previous_compact = current_compact;
    }

    if changed {
        Cow::Owned(display_text)
    } else {
        Cow::Borrowed(text)
    }
}

fn is_compact_ocr_text_character(character: char) -> bool {
    matches!(
        character,
        '\u{2E80}'..='\u{2EFF}'
            | '\u{3000}'..='\u{303F}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{31F0}'..='\u{31FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FE10}'..='\u{FE1F}'
            | '\u{FE30}'..='\u{FE4F}'
            | '\u{FF00}'..='\u{FFEF}'
            | '\u{20000}'..='\u{2FA1F}'
    )
}

fn text_overlay_font_size(bounds_rect: EguiRect, style: OcrTextOverlayStyle) -> f32 {
    (bounds_rect.height() * style.font_height_ratio).clamp(style.min_font_size, style.max_font_size)
}

fn text_overlay_bounds_rect(
    image_rect: EguiRect,
    image_size: Size,
    overlay: &TextOverlay,
) -> EguiRect {
    image_bounds_to_screen(image_rect, image_size, overlay.bounds)
}

fn image_bounds_to_screen(image_rect: EguiRect, image_size: Size, bounds: Rect) -> EguiRect {
    let scale_x = image_rect.width() / image_size.width.max(1.0);
    let scale_y = image_rect.height() / image_size.height.max(1.0);
    EguiRect::from_min_size(
        Pos2::new(
            image_rect.min.x + bounds.origin.x * scale_x,
            image_rect.min.y + bounds.origin.y * scale_y,
        ),
        Vec2::new(
            bounds.size.width.max(1.0) * scale_x,
            bounds.size.height.max(1.0) * scale_y,
        ),
    )
    .intersect(image_rect)
}
