use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Id, LayerId, Order, Painter, Pos2, Rect as EguiRect,
    Stroke, StrokeKind, Vec2,
};
use image::{DynamicImage, GenericImageView};

use crate::overlay_text::OverlayText;
use crate::{CaptureAction, ColorValueFormat, MAGNIFIER_SAMPLE_SIZE, PointerPixel};

// Rendering helpers are kept stateless so CaptureOverlayApp owns behavior, not paint details.
pub(crate) fn snapshot_color_at(snapshot: &DynamicImage, x: u32, y: u32) -> Color32 {
    let pixel = snapshot.get_pixel(x, y).0;
    Color32::from_rgb(pixel[0], pixel[1], pixel[2])
}

pub(crate) fn format_color_value(color: Color32, format: ColorValueFormat) -> String {
    match format {
        ColorValueFormat::Hex => format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b()),
        ColorValueFormat::Rgb => format!("{}, {}, {}", color.r(), color.g(), color.b()),
    }
}

pub(crate) fn draw_selection_mask(
    painter: &Painter,
    canvas: EguiRect,
    selection: EguiRect,
    mask_alpha: u8,
    border_color: Color32,
) {
    painter.rect_stroke(
        selection,
        CornerRadius::ZERO,
        Stroke::new(2.0, border_color),
        StrokeKind::Outside,
    );
    draw_resize_handles(painter, selection, border_color);

    let shade = Color32::from_black_alpha(mask_alpha);
    let top = EguiRect::from_min_max(canvas.min, Pos2::new(canvas.max.x, selection.min.y));
    let bottom = EguiRect::from_min_max(Pos2::new(canvas.min.x, selection.max.y), canvas.max);
    let left = EguiRect::from_min_max(
        Pos2::new(canvas.min.x, selection.min.y),
        Pos2::new(selection.min.x, selection.max.y),
    );
    let right = EguiRect::from_min_max(
        Pos2::new(selection.max.x, selection.min.y),
        Pos2::new(canvas.max.x, selection.max.y),
    );

    for rect in [top, bottom, left, right] {
        painter.rect_filled(rect, 0.0, shade);
    }
}

fn draw_resize_handles(painter: &Painter, selection: EguiRect, border_color: Color32) {
    let handles = [
        selection.left_top(),
        Pos2::new(selection.center().x, selection.top()),
        selection.right_top(),
        Pos2::new(selection.right(), selection.center().y),
        selection.right_bottom(),
        Pos2::new(selection.center().x, selection.bottom()),
        selection.left_bottom(),
        Pos2::new(selection.left(), selection.center().y),
    ];

    for center in handles {
        let rect = EguiRect::from_center_size(center, Vec2::splat(7.0));
        painter.rect_filled(rect, 2.0, Color32::from_black_alpha(180));
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0, border_color),
            StrokeKind::Outside,
        );
    }
}

pub(crate) fn draw_size_label(painter: &Painter, selection: EguiRect) {
    let label = format!(
        "{} x {}",
        selection.width() as i32,
        selection.height() as i32
    );
    let position = selection.min + Vec2::new(8.0, -24.0);
    let rect = EguiRect::from_min_size(position, Vec2::new(96.0, 20.0));
    painter.rect_filled(rect, 0.0, Color32::from_black_alpha(190));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::monospace(12.0),
        Color32::WHITE,
    );
}

pub(crate) fn draw_magnifier(
    painter: &Painter,
    canvas: EguiRect,
    snapshot: &Option<DynamicImage>,
    pixel: PointerPixel,
    scale: f32,
    color_format: ColorValueFormat,
) {
    let Some(snapshot) = snapshot.as_ref() else {
        return;
    };

    let cell_size = (scale * 4.0).clamp(4.0, 14.0);
    let sample_size = MAGNIFIER_SAMPLE_SIZE.max(3);
    let radius = sample_size / 2;
    let zoom_size = sample_size as f32 * cell_size;
    let info_height = 50.0;
    let panel_size = Vec2::new(zoom_size + 4.0, zoom_size + info_height + 4.0);
    let panel = floating_panel_rect(canvas, pixel.position, panel_size);
    let image_origin = panel.min + Vec2::splat(1.0);
    let image_rect = EguiRect::from_min_size(image_origin, Vec2::splat(zoom_size));

    painter.rect_filled(panel, 0, Color32::from_black_alpha(150));
    painter.rect_stroke(
        panel,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_white_alpha(70)),
        StrokeKind::Inside,
    );

    let max_x = snapshot.width().saturating_sub(1) as i32;
    let max_y = snapshot.height().saturating_sub(1) as i32;
    for row in 0..sample_size {
        for column in 0..sample_size {
            let image_x = (pixel.image_x as i32 + column - radius).clamp(0, max_x) as u32;
            let image_y = (pixel.image_y as i32 + row - radius).clamp(0, max_y) as u32;
            let color = snapshot_color_at(snapshot, image_x, image_y);
            let rect = EguiRect::from_min_size(
                image_rect.min + Vec2::new(column as f32 * cell_size, row as f32 * cell_size),
                Vec2::splat(cell_size + 0.5),
            );
            painter.rect_filled(rect, 0.0, color);
        }
    }

    let center_rect = EguiRect::from_min_size(
        image_rect.min + Vec2::new(radius as f32 * cell_size, radius as f32 * cell_size),
        Vec2::splat(cell_size),
    );
    painter.rect_stroke(
        center_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::WHITE),
        StrokeKind::Outside,
    );

    let info_top = image_rect.max.y + 1.0;
    let text_x = panel.min.x + 8.0;
    let coord_center_y = info_top + 14.0;
    let color_center_y = info_top + 34.0;
    let swatch_size = 14.0;
    let swatch = EguiRect::from_center_size(
        Pos2::new(text_x + swatch_size / 2.0, color_center_y),
        Vec2::splat(swatch_size),
    );
    let color_text_x = swatch.max.x + 7.0;

    painter.text(
        Pos2::new(text_x, coord_center_y),
        Align2::LEFT_CENTER,
        format!("({}, {})", pixel.screen_x, pixel.screen_y),
        FontId::monospace(12.0),
        Color32::from_white_alpha(225),
    );
    painter.rect_filled(swatch, 0.0, pixel.color);
    painter.rect_stroke(
        swatch,
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_white_alpha(90)),
        StrokeKind::Outside,
    );
    painter.text(
        Pos2::new(color_text_x, color_center_y),
        Align2::LEFT_CENTER,
        format_color_value(pixel.color, color_format),
        FontId::monospace(12.0),
        Color32::WHITE,
    );
}

fn floating_panel_rect(canvas: EguiRect, cursor: Pos2, size: Vec2) -> EguiRect {
    let margin = 8.0;
    let mut min = cursor + Vec2::new(24.0, 24.0);
    if min.x + size.x > canvas.max.x - margin {
        min.x = cursor.x - size.x - 18.0;
    }
    if min.y + size.y > canvas.max.y - margin {
        min.y = cursor.y - size.y - 18.0;
    }

    let min_x = canvas.min.x + margin;
    let min_y = canvas.min.y + margin;
    let max_x = (canvas.max.x - size.x - margin).max(min_x);
    let max_y = (canvas.max.y - size.y - margin).max(min_y);
    EguiRect::from_min_size(
        Pos2::new(min.x.clamp(min_x, max_x), min.y.clamp(min_y, max_y)),
        size,
    )
}

pub(crate) fn draw_toolbar(
    painter: &Painter,
    canvas: EguiRect,
    selection: EguiRect,
    border_color: Color32,
    text: OverlayText,
) {
    let toolbar = toolbar_rect(canvas, selection, text);
    painter.rect_filled(toolbar, 0.0, Color32::from_black_alpha(220));
    painter.rect_stroke(
        toolbar,
        CornerRadius::same(0),
        Stroke::new(1.0, border_color.gamma_multiply(0.7)),
        StrokeKind::Outside,
    );

    for button in toolbar_buttons(toolbar, text) {
        painter.rect_filled(button.rect, 0.0, Color32::from_white_alpha(18));
        painter.rect_stroke(
            button.rect,
            CornerRadius::same(0),
            Stroke::new(1.0, Color32::from_white_alpha(36)),
            StrokeKind::Inside,
        );
        painter.text(
            button.rect.center(),
            Align2::CENTER_CENTER,
            button.label,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}

pub(crate) fn toolbar_action_at(
    position: Pos2,
    canvas: EguiRect,
    selection: EguiRect,
    text: OverlayText,
) -> Option<CaptureAction> {
    toolbar_buttons(toolbar_rect(canvas, selection, text), text)
        .into_iter()
        .find(|button| button.rect.contains(position))
        .map(|button| button.action)
}

fn toolbar_rect(canvas: EguiRect, selection: EguiRect, text: OverlayText) -> EguiRect {
    let width = toolbar_width(text);
    let size = Vec2::new(width, 34.0);
    let x = (selection.max.x - size.x).clamp(canvas.min.x + 8.0, canvas.max.x - size.x - 8.0);
    let y = if selection.max.y + size.y + 10.0 <= canvas.max.y {
        selection.max.y + 8.0
    } else {
        selection.min.y - size.y - 8.0
    }
    .clamp(canvas.min.y + 8.0, canvas.max.y - size.y - 8.0);
    EguiRect::from_min_size(Pos2::new(x, y), size)
}

fn toolbar_width(text: OverlayText) -> f32 {
    let labels = [text.pin_action, text.copy_action, text.save_action];
    labels.iter().map(|label| button_width(label)).sum::<f32>() + 20.0
}

fn button_width(label: &str) -> f32 {
    (label.chars().count() as f32 * 12.0 + 24.0).clamp(52.0, 118.0)
}

#[derive(Debug, Clone, Copy)]
struct ToolbarButton {
    rect: EguiRect,
    label: &'static str,
    action: CaptureAction,
}

fn toolbar_buttons(toolbar: EguiRect, text: OverlayText) -> Vec<ToolbarButton> {
    let mut x = toolbar.min.x + 6.0;
    [
        (text.pin_action, CaptureAction::Pin),
        (text.copy_action, CaptureAction::Copy),
        (text.save_action, CaptureAction::Save),
    ]
    .into_iter()
    .map(|(label, action)| {
        let width = button_width(label);
        let rect = EguiRect::from_min_size(
            Pos2::new(x, toolbar.min.y + 5.0),
            Vec2::new(width, toolbar.height() - 10.0),
        );
        x += width + 4.0;
        ToolbarButton {
            rect,
            label,
            action,
        }
    })
    .collect()
}

pub(crate) fn draw_hint(painter: &Painter, canvas: EguiRect, text: &str) {
    painter.text(
        canvas.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(17.0),
        Color32::from_white_alpha(220),
    );
}

pub(crate) fn draw_error(painter: &Painter, canvas: EguiRect, error: &str) {
    let max_width = 520.0f32.min(canvas.width() - 32.0).max(240.0);
    let rect = EguiRect::from_center_size(canvas.center(), Vec2::new(max_width, 72.0));
    painter.rect_filled(rect, 0.0, Color32::from_rgb(94, 26, 28));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        error,
        FontId::proportional(13.0),
        Color32::WHITE,
    );
}

pub(crate) fn draw_pin_border(painter: &Painter, canvas: EguiRect) {
    let layer = LayerId::new(Order::Foreground, Id::new("pin-border"));
    let painter = painter.clone().with_layer_id(layer);
    painter.rect_stroke(
        canvas.shrink(0.5),
        CornerRadius::ZERO,
        Stroke::new(1.0, Color32::from_black_alpha(120)),
        StrokeKind::Inside,
    );
}
