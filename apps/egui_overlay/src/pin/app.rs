use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use eframe::egui::{
    self, Color32, ColorImage, Context, Id, Key, Painter, PointerButton, Pos2, Rect as EguiRect,
    Sense, TextureHandle, TextureOptions, Vec2, ViewportCommand, WindowLevel,
};
use eframe::{App, CreationContext, Frame};
use image::DynamicImage;
use shared_models::{
    OcrProvider, OcrResult, OcrTextBlock, Point, Rect, Size, TextOverlay, TranslateProvider,
};

use crate::capture::hotkeys::{command_shift_shortcut_pressed, copy_shortcut_pressed};
use crate::capture::paint::{draw_error, draw_pin_border};
use crate::capture::snapshot_io::capture_file_name;
use crate::pin::ocr::{PinOcrRequest, parse_ocr_provider, recognize_pin_image};
use crate::pin::text_overlay::{
    OcrTextOverlayStyle, draw_text_overlay, ocr_block_interaction_rect,
};
use crate::pin::toolbar::{
    PinToolbarAction, PinToolbarEdge, PinToolbarState, draw_pin_toolbar, pin_image_rect,
    pin_toolbar_action_at, pin_toolbar_bounds, pin_toolbar_rect, pin_window_size_for_image,
};
use crate::pin::translate::{
    PinBlockTranslateRequest, PinBlockTranslation, PinTranslatableBlock, parse_translate_provider,
    translate_pin_blocks,
};
use crate::pin::window::{
    PIN_MIN_OPACITY, PIN_OPACITY_STEP, PinWindowSizing, clamp_pin_window_size,
    current_viewport_rect, draw_pin_status, fit_pin_image_size_to_canvas,
};
use crate::runtime::cli::CliArgs;
use crate::runtime::fonts::install_system_fonts;
use crate::runtime::text::OverlayText;
pub(crate) struct PinWindowApp {
    text: OverlayText,
    image_path: Option<PathBuf>,
    texture: Option<TextureHandle>,
    image_size: Vec2,
    image_display_size: Vec2,
    sizing: PinWindowSizing,
    error: Option<String>,
    opacity: f32,
    zoom_step: f32,
    toolbar_edge: Option<PinToolbarEdge>,
    pending_toolbar_action: Option<PinToolbarAction>,
    ocr_provider: OcrProvider,
    ocr_language_hint: Option<String>,
    ocr_default_model_id: Option<String>,
    ocr_models_registry: Option<PathBuf>,
    ocr_receiver: Option<mpsc::Receiver<Result<OcrResult, String>>>,
    ocr_result: Option<OcrResult>,
    ocr_status: Option<String>,
    selected_ocr_block: Option<usize>,
    translate_provider: TranslateProvider,
    translate_target_language: String,
    translate_segmentation_mode: PinTranslationSegmentationMode,
    smart_merge_settings: SmartMergeSettings,
    translate_default_model_id: Option<String>,
    translate_receiver: Option<mpsc::Receiver<Result<Vec<PinBlockTranslation>, String>>>,
    block_translations: Vec<PinBlockTranslation>,
    translate_after_ocr: bool,
    ocr_text_style: OcrTextOverlayStyle,
}

impl PinWindowApp {
    pub(crate) fn new(creation_context: &CreationContext<'_>, args: CliArgs) -> Self {
        install_system_fonts(&creation_context.egui_ctx);
        let text = OverlayText::new(args.language);
        let initial_image_size = Vec2::new(args.width, args.height);
        let sizing = PinWindowSizing::new(args.pin_min_width, args.pin_min_height);
        let mut app = Self {
            text,
            image_path: args.image,
            texture: None,
            image_size: initial_image_size,
            image_display_size: clamp_pin_window_size(initial_image_size, sizing),
            sizing,
            error: None,
            opacity: args.pin_opacity,
            zoom_step: args.pin_zoom_step,
            toolbar_edge: None,
            pending_toolbar_action: None,
            ocr_provider: parse_ocr_provider(&args.ocr_provider),
            ocr_language_hint: args.ocr_language_hint,
            ocr_default_model_id: args.ocr_default_model_id,
            ocr_models_registry: args.ocr_models_registry,
            ocr_receiver: None,
            ocr_result: None,
            ocr_status: None,
            selected_ocr_block: None,
            translate_provider: parse_translate_provider(&args.translate_provider),
            translate_target_language: args.translate_target_language,
            translate_segmentation_mode: PinTranslationSegmentationMode::from_name(
                &args.translate_segmentation_mode,
            ),
            smart_merge_settings: SmartMergeSettings {
                edge_tolerance_lines: args.smart_merge_edge_tolerance_lines,
                loose_edge_tolerance_lines: args.smart_merge_loose_edge_tolerance_lines,
                height_ratio_limit: args.smart_merge_height_ratio_limit,
                longer_line_ratio: args.smart_merge_longer_line_ratio,
                short_last_line_ratio: args.smart_merge_short_last_line_ratio,
                inline_label_max_chars: args.smart_merge_inline_label_max_chars,
            },
            translate_default_model_id: args.translate_default_model_id,
            translate_receiver: None,
            block_translations: Vec::new(),
            translate_after_ocr: false,
            ocr_text_style: OcrTextOverlayStyle::new(
                args.ocr_text_font_height_ratio,
                args.ocr_text_min_font_size,
                args.ocr_text_max_font_size,
                args.ocr_text_padding_x,
                args.ocr_text_padding_y,
                args.ocr_text_interaction_padding_x,
                args.ocr_text_interaction_padding_y,
            ),
        };
        let level = if args.pin_always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        };
        creation_context
            .egui_ctx
            .send_viewport_cmd(ViewportCommand::WindowLevel(level));
        app.load_texture(&creation_context.egui_ctx);
        app
    }

    fn load_texture(&mut self, ctx: &Context) {
        let Some(path) = &self.image_path else {
            log::error!("pin window missing image path");
            self.error = Some(self.text.missing_pin_image.to_owned());
            return;
        };

        log::info!("pin window loading image {}", path.display());
        match image::open(path) {
            Ok(image) => {
                let image = image.to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                self.image_size = Vec2::new(size[0] as f32, size[1] as f32);
                self.image_display_size = clamp_pin_window_size(self.image_size, self.sizing);
                self.texture = Some(ctx.load_texture(
                    "pinned-image",
                    ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                    TextureOptions::LINEAR,
                ));
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.image_display_size));
                log::info!("pin window image loaded size={}x{}", size[0], size[1]);
            }
            Err(error) => {
                log::error!(
                    "pin window failed to load image {}: {error}",
                    path.display()
                );
                self.error = Some(format!("{}: {error}", self.text.pin_load_failed));
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        if ctx.input(|input| command_shift_shortcut_pressed(input, Key::C)) {
            self.copy_all_ocr_text();
            ctx.request_repaint();
        } else if ctx.input(copy_shortcut_pressed) {
            self.copy_selected_ocr_text_or_image();
            ctx.request_repaint();
        }

        let (scroll, ctrl_down, viewport_rect, pointer_pos) = ctx.input(|input| {
            (
                input.raw_scroll_delta.y,
                input.modifiers.ctrl,
                input.viewport().inner_rect.or(input.viewport().outer_rect),
                input.pointer.hover_pos(),
            )
        });
        if scroll.abs() <= f32::EPSILON {
            return;
        }

        if ctrl_down {
            let delta = if scroll > 0.0 {
                PIN_OPACITY_STEP
            } else {
                -PIN_OPACITY_STEP
            };
            self.opacity = (self.opacity + delta).clamp(PIN_MIN_OPACITY, 1.0);
            ctx.request_repaint();
            return;
        }

        self.zoom_with_wheel(ctx, scroll, viewport_rect, pointer_pos);
    }

    fn zoom_with_wheel(
        &mut self,
        ctx: &Context,
        scroll: f32,
        viewport_rect: Option<EguiRect>,
        pointer_pos: Option<Pos2>,
    ) {
        let Some(viewport_rect) = viewport_rect else {
            return;
        };
        let current_size = self.image_display_size;
        if current_size.x <= 0.0 || current_size.y <= 0.0 {
            return;
        }

        let zoom_factor = if scroll > 0.0 {
            1.0 + self.zoom_step
        } else {
            1.0 / (1.0 + self.zoom_step)
        };
        let new_size = clamp_pin_window_size(current_size * zoom_factor, self.sizing);
        if (new_size - current_size).length_sq() <= f32::EPSILON {
            return;
        }

        let canvas = EguiRect::from_min_size(Pos2::ZERO, viewport_rect.size());
        let image_rect = self.image_rect(canvas);
        let anchor = pointer_pos.unwrap_or(image_rect.center());
        let anchor_fraction = Vec2::new(
            ((anchor.x - image_rect.min.x) / current_size.x).clamp(0.0, 1.0),
            ((anchor.y - image_rect.min.y) / current_size.y).clamp(0.0, 1.0),
        );

        let state = self.toolbar_state();
        let new_window_size = pin_window_size_for_image(new_size, self.toolbar_edge, state);
        let new_canvas = EguiRect::from_min_size(Pos2::ZERO, new_window_size);
        let new_image_rect = pin_image_rect(new_canvas, new_size, self.toolbar_edge, state);
        let current_anchor = image_rect.min + current_size * anchor_fraction;
        let new_anchor = new_image_rect.min + new_size * anchor_fraction;
        let new_window_min = viewport_rect.min + current_anchor.to_vec2() - new_anchor.to_vec2();

        log::info!(
            "pin zoom toolbar_edge={:?} image_size={}x{} new_image_size={}x{} window_size={}x{}",
            self.toolbar_edge,
            current_size.x,
            current_size.y,
            new_size.x,
            new_size.y,
            new_window_size.x,
            new_window_size.y
        );

        self.image_display_size = new_size;
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(new_window_min));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(new_window_size));
        ctx.request_repaint();
    }

    fn handle_canvas_response(
        &mut self,
        ctx: &Context,
        response: &egui::Response,
        canvas: EguiRect,
        image_rect: EguiRect,
    ) {
        if response.secondary_clicked() {
            if self.toolbar_edge.is_some() {
                self.hide_toolbar(ctx, canvas);
                return;
            }

            if let Some(position) = ctx.input(|input| input.pointer.interact_pos()) {
                self.show_toolbar(ctx, canvas, PinToolbarEdge::nearest(image_rect, position));
            }
            return;
        }

        if let Some(action) = self.clicked_toolbar_action(ctx, canvas, image_rect) {
            self.run_toolbar_action(ctx, action);
            return;
        }

        let pointer_pos = ctx.input(|input| input.pointer.interact_pos());
        let pointer_over_toolbar =
            pointer_pos.is_some_and(|position| self.toolbar_contains(position, canvas, image_rect));
        let pointer_over_ocr =
            pointer_pos.is_some_and(|position| self.ocr_block_at(position, image_rect).is_some());
        let pointer_over_image = pointer_pos.is_some_and(|position| image_rect.contains(position));

        if response.double_clicked_by(PointerButton::Primary)
            && pointer_over_image
            && !pointer_over_toolbar
            && !pointer_over_ocr
        {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        if response.clicked_by(PointerButton::Primary) && pointer_over_ocr {
            self.selected_ocr_block =
                pointer_pos.and_then(|position| self.ocr_block_at(position, image_rect));
            ctx.request_repaint();
            return;
        }

        if response.clicked_by(PointerButton::Primary) && !pointer_over_toolbar {
            self.selected_ocr_block = None;
        }

        if response.dragged_by(PointerButton::Primary) && !pointer_over_toolbar && !pointer_over_ocr
        {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
    }

    fn clicked_toolbar_action(
        &self,
        ctx: &Context,
        canvas: EguiRect,
        image_rect: EguiRect,
    ) -> Option<PinToolbarAction> {
        if !ctx.input(|input| input.pointer.button_clicked(PointerButton::Primary)) {
            return None;
        }

        let position = ctx.input(|input| input.pointer.interact_pos())?;
        pin_toolbar_action_at(
            position,
            canvas,
            self.toolbar_rect(canvas, image_rect)?,
            self.toolbar_state(),
        )
    }

    fn run_toolbar_action(&mut self, ctx: &Context, action: PinToolbarAction) {
        self.pending_toolbar_action = Some(action);

        match action {
            PinToolbarAction::RunOcr => {
                self.run_ocr_for_pin(ctx);
            }
            PinToolbarAction::CloseOcr => self.close_ocr(ctx),
            PinToolbarAction::CopyImage => self.copy_pin_image(),
            PinToolbarAction::CopySelectedText => self.copy_selected_ocr_text(),
            PinToolbarAction::CopyAllText => self.copy_all_ocr_text(),
            PinToolbarAction::SaveImage => self.save_pin_image(),
            PinToolbarAction::Translate => {
                self.run_translation_for_pin(ctx);
            }
            PinToolbarAction::Close => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }

        ctx.request_repaint();
    }

    fn show_toolbar(&mut self, ctx: &Context, canvas: EguiRect, edge: PinToolbarEdge) {
        let image_rect = self.image_rect(canvas);
        self.image_display_size = image_rect.size();
        self.toolbar_edge = Some(edge);
        self.resize_window_for_image_rect(ctx, image_rect, Some(edge));
    }

    fn hide_toolbar(&mut self, ctx: &Context, canvas: EguiRect) {
        let Some(edge) = self.toolbar_edge else {
            return;
        };
        let image_rect = self.image_rect(canvas);
        self.image_display_size = image_rect.size();
        self.toolbar_edge = None;
        self.resize_window_for_image_rect(ctx, image_rect, Some(edge));
    }

    fn resize_window_for_image_rect(
        &self,
        ctx: &Context,
        current_image_rect: EguiRect,
        previous_edge: Option<PinToolbarEdge>,
    ) {
        let Some(viewport_rect) = current_viewport_rect(ctx) else {
            ctx.request_repaint();
            return;
        };

        let current_image_screen_min = viewport_rect.min + current_image_rect.min.to_vec2();
        let new_size = pin_window_size_for_image(
            self.image_display_size,
            self.toolbar_edge,
            self.toolbar_state(),
        );
        let new_canvas = EguiRect::from_min_size(Pos2::ZERO, new_size);
        let new_image_rect = pin_image_rect(
            new_canvas,
            self.image_display_size,
            self.toolbar_edge,
            self.toolbar_state(),
        );
        let new_window_min = current_image_screen_min - new_image_rect.min.to_vec2();

        log::info!(
            "pin toolbar layout previous_edge={:?} edge={:?} image_size={}x{} window_size={}x{}",
            previous_edge,
            self.toolbar_edge,
            self.image_display_size.x,
            self.image_display_size.y,
            new_size.x,
            new_size.y
        );
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(new_window_min));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(new_size));
        ctx.request_repaint();
    }

    fn sync_display_size_to_canvas(&mut self, canvas: EguiRect) {
        if self.toolbar_edge.is_some() {
            return;
        }

        let fitted_size = fit_pin_image_size_to_canvas(self.image_size, canvas.size(), self.sizing);
        if (self.image_display_size - fitted_size).length_sq() > f32::EPSILON {
            self.image_display_size = fitted_size;
        }
    }

    fn image_rect(&self, canvas: EguiRect) -> EguiRect {
        pin_image_rect(
            canvas,
            self.image_display_size,
            self.toolbar_edge,
            self.toolbar_state(),
        )
    }

    fn toolbar_rect(&self, canvas: EguiRect, image_rect: EguiRect) -> Option<EguiRect> {
        self.toolbar_edge
            .map(|edge| pin_toolbar_rect(canvas, image_rect, edge, self.toolbar_state()))
    }

    fn toolbar_contains(&self, position: Pos2, canvas: EguiRect, image_rect: EguiRect) -> bool {
        self.toolbar_rect(canvas, image_rect)
            .is_some_and(|toolbar| {
                pin_toolbar_bounds(canvas, toolbar, self.toolbar_state())
                    .expand(4.0)
                    .contains(position)
            })
    }

    fn draw_toolbar(&self, painter: &Painter, canvas: EguiRect, image_rect: EguiRect) {
        let Some(toolbar) = self.toolbar_rect(canvas, image_rect) else {
            return;
        };

        draw_pin_toolbar(painter, canvas, toolbar, self.toolbar_state());
    }

    fn toolbar_state(&self) -> PinToolbarState {
        PinToolbarState {
            text: self.text,
            ocr_active: self.ocr_result.is_some() || self.ocr_receiver.is_some(),
            has_ocr_text: self.all_ocr_text().is_some(),
            has_selected_ocr_text: self.selected_ocr_text().is_some(),
        }
    }

    fn draw_ocr_overlays(&self, painter: &Painter, image_rect: EguiRect) {
        let Some(result) = &self.ocr_result else {
            return;
        };

        let image_size = Size::new(self.image_size.x.max(1.0), self.image_size.y.max(1.0));
        for (index, block) in result.blocks.iter().enumerate() {
            let translation = self.translation_for_block(index);
            let should_draw_translation = translation.is_some_and(|translation| {
                self.translation_display_block_index(translation) == Some(index)
            });
            if translation.is_some() && !should_draw_translation {
                continue;
            }

            let overlay = TextOverlay {
                text: if should_draw_translation {
                    translation
                        .map(|translation| translation.translated_text.clone())
                        .unwrap_or_else(|| block.text.clone())
                } else {
                    block.text.clone()
                },
                language: if should_draw_translation {
                    translation
                        .map(|translation| translation.target_language.clone())
                        .or_else(|| block.language.clone())
                } else {
                    block.language.clone()
                },
                bounds: block.bounds,
                role: if should_draw_translation {
                    shared_models::TextOverlayRole::Translation
                } else {
                    shared_models::TextOverlayRole::Ocr
                },
                confidence: block.confidence,
            };
            draw_text_overlay(
                painter,
                image_rect,
                image_size,
                &overlay,
                self.ocr_text_style,
                self.selected_ocr_block == Some(index),
                ocr_text_right_limit_x(result, index, image_rect, image_size),
            );
        }
    }

    fn translation_display_block_index(&self, translation: &PinBlockTranslation) -> Option<usize> {
        if self.uses_block_replacement() {
            return Some(translation.index);
        }

        translation.block_indices.first().copied()
    }

    fn run_ocr_for_pin(&mut self, ctx: &Context) {
        if self.ocr_receiver.is_some() {
            return;
        }

        let Some(path) = self.image_path.clone() else {
            self.ocr_result = None;
            self.ocr_status = Some(self.text.missing_pin_image.to_owned());
            return;
        };

        self.ocr_status = Some("OCR running...".to_owned());
        self.ocr_result = None;
        self.selected_ocr_block = None;
        self.block_translations.clear();

        let request = PinOcrRequest {
            path,
            provider: self.ocr_provider.clone(),
            language_hint: self.ocr_language_hint.clone(),
            default_model_id: self.ocr_default_model_id.clone(),
            models_registry: self.ocr_models_registry.clone(),
            load_error_prefix: self.text.pin_load_failed.to_owned(),
        };
        let (sender, receiver) = mpsc::channel();
        let repaint_ctx = ctx.clone();
        thread::spawn(move || {
            let result = recognize_pin_image(request);
            let _ = sender.send(result);
            repaint_ctx.request_repaint();
        });
        self.ocr_receiver = Some(receiver);
    }

    fn run_translation_for_pin(&mut self, ctx: &Context) {
        if self.translate_receiver.is_some() {
            return;
        }

        let Some(result) = self.ocr_result.as_ref() else {
            self.translate_after_ocr = true;
            self.run_ocr_for_pin(ctx);
            self.ocr_status = Some("OCR running before translation...".to_owned());
            return;
        };
        let blocks = self.translatable_blocks_for_result(result);
        if blocks.is_empty() {
            self.translate_after_ocr = true;
            self.run_ocr_for_pin(ctx);
            self.ocr_status = Some("OCR running before translation...".to_owned());
            return;
        }

        self.translate_after_ocr = false;
        self.ocr_status = Some("Translation running...".to_owned());
        self.block_translations.clear();
        let request = PinBlockTranslateRequest {
            blocks,
            target_language: self.translate_target_language.clone(),
            provider: self.translate_provider.clone(),
            default_model_id: self.translate_default_model_id.clone(),
            models_registry: self.ocr_models_registry.clone(),
        };
        let (sender, receiver) = mpsc::channel();
        let repaint_ctx = ctx.clone();
        thread::spawn(move || {
            let result = translate_pin_blocks(request);
            let _ = sender.send(result);
            repaint_ctx.request_repaint();
        });
        self.translate_receiver = Some(receiver);
    }

    fn close_ocr(&mut self, ctx: &Context) {
        let previous_edge = self.toolbar_edge;
        self.ocr_receiver = None;
        self.ocr_result = None;
        self.ocr_status = None;
        self.selected_ocr_block = None;
        self.translate_receiver = None;
        self.block_translations.clear();
        self.translate_after_ocr = false;
        self.resize_toolbar_window_from_viewport(ctx, previous_edge);
    }

    fn drain_ocr_result(&mut self, ctx: &Context) {
        let Some(receiver) = &self.ocr_receiver else {
            return;
        };

        let Ok(result) = receiver.try_recv() else {
            return;
        };

        self.ocr_receiver = None;
        match result {
            Ok(result) => {
                log::info!(
                    "pin OCR completed image={} blocks={} text_chars={}",
                    result.image_id.0,
                    result.blocks.len(),
                    result.plain_text.chars().count()
                );
                self.ocr_status = None;
                self.selected_ocr_block = None;
                self.block_translations.clear();
                self.ocr_result = Some(result);
                if self.translate_after_ocr {
                    self.run_translation_for_pin(ctx);
                }
            }
            Err(error) => {
                log::error!("pin OCR failed: {error}");
                self.ocr_result = None;
                self.selected_ocr_block = None;
                self.ocr_status = Some(error);
            }
        }
        self.resize_toolbar_window_from_viewport(ctx, self.toolbar_edge);
    }

    fn drain_translation_result(&mut self, ctx: &Context) {
        let Some(receiver) = &self.translate_receiver else {
            return;
        };

        let Ok(result) = receiver.try_recv() else {
            return;
        };

        self.translate_receiver = None;
        self.translate_after_ocr = false;
        match result {
            Ok(translations) => {
                let translated_chars = translations
                    .iter()
                    .map(|translation| translation.translated_text.chars().count())
                    .sum::<usize>();
                log::info!(
                    "pin translation completed units={} translated_chars={}",
                    translations.len(),
                    translated_chars
                );
                self.ocr_status = None;
                self.block_translations = translations;
            }
            Err(error) => {
                log::error!("pin translation failed: {error}");
                self.block_translations.clear();
                self.ocr_status = Some(error);
            }
        }
        self.resize_toolbar_window_from_viewport(ctx, self.toolbar_edge);
    }

    fn resize_toolbar_window_from_viewport(
        &self,
        ctx: &Context,
        previous_edge: Option<PinToolbarEdge>,
    ) {
        let Some(edge) = self.toolbar_edge else {
            ctx.request_repaint();
            return;
        };
        let Some(viewport_rect) = current_viewport_rect(ctx) else {
            ctx.request_repaint();
            return;
        };
        let canvas = EguiRect::from_min_size(Pos2::ZERO, viewport_rect.size());
        let image_rect = self.image_rect(canvas);
        self.resize_window_for_image_rect(ctx, image_rect, previous_edge.or(Some(edge)));
    }

    fn ocr_block_at(&self, position: Pos2, image_rect: EguiRect) -> Option<usize> {
        let result = self.ocr_result.as_ref()?;
        let image_size = Size::new(self.image_size.x.max(1.0), self.image_size.y.max(1.0));
        result
            .blocks
            .iter()
            .enumerate()
            .rev()
            .find(|(_, block)| {
                !block.text.trim().is_empty()
                    && ocr_block_interaction_rect(
                        image_rect,
                        image_size,
                        block.bounds,
                        self.ocr_text_style,
                    )
                    .contains(position)
            })
            .map(|(index, _)| index)
    }

    fn selected_ocr_text(&self) -> Option<String> {
        let result = self.ocr_result.as_ref()?;
        if let Some(index) = self.selected_ocr_block {
            if let Some(translation) = self.translation_for_block(index) {
                let text = translation.translated_text.trim();
                if !text.is_empty() {
                    return Some(text.to_owned());
                }
            }
            return result
                .blocks
                .get(index)
                .map(|block| block.text.trim())
                .filter(|text| !text.is_empty())
                .map(str::to_owned);
        }

        None
    }

    fn all_ocr_text(&self) -> Option<String> {
        if !self.block_translations.is_empty() {
            let text = self
                .block_translations
                .iter()
                .map(|translation| translation.translated_text.trim())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            return (!text.is_empty()).then_some(text);
        }

        let result = self.ocr_result.as_ref()?;
        let text = result.plain_text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    fn block_translation(&self, index: usize) -> Option<&PinBlockTranslation> {
        self.block_translations
            .iter()
            .find(|translation| translation.index == index)
    }

    fn translation_for_block(&self, index: usize) -> Option<&PinBlockTranslation> {
        if self.uses_block_replacement() {
            return self.block_translation(index);
        }

        self.block_translations
            .iter()
            .find(|translation| translation.block_indices.contains(&index))
    }

    fn uses_block_replacement(&self) -> bool {
        self.translate_segmentation_mode == PinTranslationSegmentationMode::BlockReplace
    }

    fn translatable_blocks_for_result(&self, result: &OcrResult) -> Vec<PinTranslatableBlock> {
        match self.translate_segmentation_mode {
            PinTranslationSegmentationMode::BlockReplace => {
                translatable_blocks_by_ocr_block(result)
            }
            PinTranslationSegmentationMode::SmartMerge => {
                translatable_blocks_by_smart_merge(result, self.smart_merge_settings)
            }
            PinTranslationSegmentationMode::FullRegion => {
                translatable_blocks_by_full_region(result)
            }
        }
    }

    fn copy_selected_ocr_text_or_image(&mut self) {
        if self.selected_ocr_text().is_some() {
            self.copy_selected_ocr_text();
        } else {
            self.copy_pin_image();
        }
    }

    fn copy_selected_ocr_text(&mut self) {
        let Some(text) = self.selected_ocr_text() else {
            return;
        };

        self.copy_text_to_clipboard(text);
    }

    fn copy_all_ocr_text(&mut self) {
        let Some(text) = self.all_ocr_text() else {
            return;
        };

        self.copy_text_to_clipboard(text);
    }

    fn copy_text_to_clipboard(&mut self, text: String) {
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text.clone())) {
            Ok(()) => {
                log::info!("pin OCR text copied chars={}", text.chars().count());
                self.ocr_status = Some("Copied".to_owned());
            }
            Err(error) => {
                let message = format!("{}: {error}", self.text.copy_failed);
                log::error!("{message}");
                self.ocr_status = Some(message);
            }
        }
    }

    fn copy_pin_image(&mut self) {
        let Some(image) = self.load_pin_image_for_action() else {
            return;
        };

        let rgba = image.to_rgba8();
        let clipboard_image = arboard::ImageData {
            width: rgba.width() as usize,
            height: rgba.height() as usize,
            bytes: Cow::Owned(rgba.into_raw()),
        };
        match arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_image(clipboard_image))
        {
            Ok(()) => {
                log::info!("pin image copied");
                self.ocr_status = Some("Copied".to_owned());
            }
            Err(error) => {
                let message = format!("{}: {error}", self.text.copy_failed);
                log::error!("{message}");
                self.ocr_status = Some(message);
            }
        }
    }

    fn save_pin_image(&mut self) {
        let Some(image) = self.load_pin_image_for_action() else {
            return;
        };

        let default_name = capture_file_name();
        let image_path = match platform_runtime::create_platform()
            .file_dialog()
            .save_png_path(&default_name)
        {
            Ok(Some(path)) => path,
            Ok(None) => {
                log::info!("pin save path prompt canceled");
                return;
            }
            Err(error) => {
                let message = format!("{}: {error}", self.text.save_failed);
                log::error!("{message}");
                self.ocr_status = Some(message);
                return;
            }
        };

        if let Some(parent) = image_path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                let message = format!("{}: {error}", self.text.save_failed);
                log::error!("{message}");
                self.ocr_status = Some(message);
                return;
            }
        }

        match image.save(&image_path) {
            Ok(()) => {
                log::info!("saved pin image to {}", image_path.display());
                self.ocr_status = Some("Saved".to_owned());
            }
            Err(error) => {
                let message = format!("{}: {error}", self.text.save_failed);
                log::error!("{message}");
                self.ocr_status = Some(message);
            }
        }
    }

    fn load_pin_image_for_action(&mut self) -> Option<DynamicImage> {
        let Some(path) = &self.image_path else {
            self.ocr_status = Some(self.text.missing_pin_image.to_owned());
            return None;
        };

        match image::open(path) {
            Ok(image) => Some(image),
            Err(error) => {
                let message = format!("{}: {error}", self.text.pin_load_failed);
                log::error!("{message}");
                self.ocr_status = Some(message);
                None
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinTranslationSegmentationMode {
    SmartMerge,
    BlockReplace,
    FullRegion,
}

impl PinTranslationSegmentationMode {
    fn from_name(value: &str) -> Self {
        match value {
            "block-replace" => Self::BlockReplace,
            "full-region" => Self::FullRegion,
            _ => Self::SmartMerge,
        }
    }
}

fn translatable_blocks_by_ocr_block(result: &OcrResult) -> Vec<PinTranslatableBlock> {
    result
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| !block.text.trim().is_empty())
        .map(|(index, block)| PinTranslatableBlock {
            index,
            block_indices: vec![index],
            bounds: block.bounds,
            text: block.text.clone(),
            source_language: block.language.clone(),
        })
        .collect()
}

fn translatable_blocks_by_full_region(result: &OcrResult) -> Vec<PinTranslatableBlock> {
    let blocks = result
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, block)| !block.text.trim().is_empty())
        .collect::<Vec<_>>();
    if blocks.is_empty() {
        return Vec::new();
    }

    vec![translatable_block_from_group(0, &blocks)]
}

fn translatable_blocks_by_smart_merge(
    result: &OcrResult,
    settings: SmartMergeSettings,
) -> Vec<PinTranslatableBlock> {
    let mut units = Vec::new();
    let mut group: Vec<(usize, &OcrTextBlock)> = Vec::new();

    for (index, block) in result.blocks.iter().enumerate() {
        if block.text.trim().is_empty() {
            continue;
        }

        let should_start_new = group
            .last()
            .is_some_and(|_| !should_merge_ocr_block_into_group(&group, block, settings));
        if should_start_new {
            units.push(translatable_block_from_group(units.len(), &group));
            group.clear();
        }
        group.push((index, block));
    }

    if !group.is_empty() {
        units.push(translatable_block_from_group(units.len(), &group));
    }

    units
}

fn translatable_block_from_group(
    index: usize,
    group: &[(usize, &OcrTextBlock)],
) -> PinTranslatableBlock {
    let first = group
        .first()
        .expect("translation group should contain at least one OCR block");
    let bounds = group
        .iter()
        .skip(1)
        .fold(first.1.bounds, |bounds, (_, block)| {
            union_rect(bounds, block.bounds)
        });
    let text = group.iter().fold(String::new(), |mut text, (_, block)| {
        append_ocr_fragment(&mut text, block.text.trim());
        text
    });
    let source_language = group.iter().find_map(|(_, block)| block.language.clone());

    PinTranslatableBlock {
        index,
        block_indices: group.iter().map(|(block_index, _)| *block_index).collect(),
        bounds,
        text,
        source_language,
    }
}

#[derive(Debug, Clone, Copy)]
struct SmartMergeSettings {
    edge_tolerance_lines: f32,
    loose_edge_tolerance_lines: f32,
    height_ratio_limit: f32,
    longer_line_ratio: f32,
    short_last_line_ratio: f32,
    inline_label_max_chars: usize,
}

impl Default for SmartMergeSettings {
    fn default() -> Self {
        Self {
            edge_tolerance_lines: 1.35,
            loose_edge_tolerance_lines: 2.4,
            height_ratio_limit: 1.5,
            longer_line_ratio: 1.35,
            short_last_line_ratio: 0.72,
            inline_label_max_chars: 32,
        }
    }
}

fn should_merge_ocr_block_into_group(
    group: &[(usize, &OcrTextBlock)],
    next: &OcrTextBlock,
    settings: SmartMergeSettings,
) -> bool {
    let Some((_, previous)) = group.last() else {
        return true;
    };

    let previous_text = previous.text.trim();
    let next_text = next.text.trim();
    if previous_text.is_empty() || next_text.is_empty() {
        return false;
    }

    let previous_bounds = previous.bounds;
    let next_bounds = next.bounds;
    let line_height = merged_line_height(previous_bounds, next_bounds);
    let vertical_gap = next_bounds.origin.y - previous_bounds.max_y();
    let same_line = vertical_gap.abs() <= line_height * 0.55
        && vertical_overlap_ratio(previous_bounds, next_bounds) >= 0.35
        && next_bounds.origin.x >= previous_bounds.origin.x - line_height;
    if same_line {
        return true;
    }

    if ends_sentence(previous_text) {
        return false;
    }

    if vertical_gap < -line_height * 0.25 || vertical_gap > line_height * 1.25 {
        return false;
    }

    if !similar_ocr_line_height(previous_bounds, next_bounds, settings) {
        return false;
    }

    if group_ends_with_short_line(group, line_height, settings) {
        return false;
    }

    if starts_with_inline_label(next_text, settings) {
        return false;
    }

    let edge_tolerance = smart_merge_edge_tolerance(line_height, settings);
    let left_aligned = group_edge_aligned_left(group, next_bounds, edge_tolerance);
    let right_aligned = group_edge_aligned_right(group, next_bounds, edge_tolerance);
    if !left_aligned && !right_aligned {
        return false;
    }

    if group.len() == 1
        && next_line_is_much_longer(previous_bounds, next_bounds, line_height, settings)
    {
        return false;
    }

    cross_line_width_is_compatible(
        group,
        next_bounds,
        left_aligned,
        right_aligned,
        line_height,
        settings,
    )
}

fn merged_line_height(previous: Rect, next: Rect) -> f32 {
    previous.size.height.max(next.size.height).max(1.0)
}

fn similar_ocr_line_height(previous: Rect, next: Rect, settings: SmartMergeSettings) -> bool {
    let min_height = previous.size.height.min(next.size.height).max(1.0);
    let max_height = previous.size.height.max(next.size.height).max(1.0);
    max_height / min_height <= settings.height_ratio_limit
}

fn smart_merge_edge_tolerance(line_height: f32, settings: SmartMergeSettings) -> f32 {
    (line_height * settings.edge_tolerance_lines).max(8.0)
}

fn smart_merge_loose_edge_tolerance(line_height: f32, settings: SmartMergeSettings) -> f32 {
    (line_height * settings.loose_edge_tolerance_lines).max(12.0)
}

fn group_edge_aligned_left(
    group: &[(usize, &OcrTextBlock)],
    next_bounds: Rect,
    tolerance: f32,
) -> bool {
    group
        .iter()
        .any(|(_, block)| (block.bounds.origin.x - next_bounds.origin.x).abs() <= tolerance)
}

fn group_edge_aligned_right(
    group: &[(usize, &OcrTextBlock)],
    next_bounds: Rect,
    tolerance: f32,
) -> bool {
    group
        .iter()
        .any(|(_, block)| (block.bounds.max_x() - next_bounds.max_x()).abs() <= tolerance)
}

fn group_reference_width(group: &[(usize, &OcrTextBlock)]) -> f32 {
    group
        .iter()
        .map(|(_, block)| block.bounds.size.width.max(0.0))
        .fold(0.0, f32::max)
}

fn group_ends_with_short_line(
    group: &[(usize, &OcrTextBlock)],
    line_height: f32,
    settings: SmartMergeSettings,
) -> bool {
    if group.len() < 2 {
        return false;
    }

    let last_width = group
        .last()
        .map(|(_, block)| block.bounds.size.width.max(0.0))
        .unwrap_or_default();
    let reference_width = group
        .iter()
        .take(group.len() - 1)
        .map(|(_, block)| block.bounds.size.width.max(0.0))
        .fold(0.0, f32::max);

    reference_width > line_height * 4.0
        && reference_width - last_width > line_height * 3.0
        && last_width <= reference_width * settings.short_last_line_ratio
}

fn next_line_is_much_longer(
    previous: Rect,
    next: Rect,
    line_height: f32,
    settings: SmartMergeSettings,
) -> bool {
    let width_growth = next.size.width - previous.size.width;
    width_growth > line_height * 3.0
        && next.size.width > previous.size.width * settings.longer_line_ratio
}

fn cross_line_width_is_compatible(
    group: &[(usize, &OcrTextBlock)],
    next_bounds: Rect,
    left_aligned: bool,
    right_aligned: bool,
    line_height: f32,
    settings: SmartMergeSettings,
) -> bool {
    if next_bounds.size.width < line_height {
        return false;
    }

    let Some((_, previous)) = group.last() else {
        return true;
    };
    let previous_bounds = previous.bounds;
    let loose_tolerance = smart_merge_loose_edge_tolerance(line_height, settings);
    let left_delta = (next_bounds.origin.x - previous_bounds.origin.x).abs();
    let right_delta = (next_bounds.max_x() - previous_bounds.max_x()).abs();
    let reference_width = group_reference_width(group).max(previous_bounds.size.width);
    let short_continuation = group.len() >= 2
        && next_bounds.size.width <= reference_width * settings.short_last_line_ratio
        && reference_width - next_bounds.size.width > line_height * 2.0;

    if left_aligned {
        return right_delta <= loose_tolerance
            || short_continuation
            || next_bounds.max_x() <= previous_bounds.max_x() + loose_tolerance;
    }

    if right_aligned {
        return left_delta <= loose_tolerance
            || next_bounds.origin.x >= previous_bounds.origin.x - loose_tolerance;
    }

    false
}

fn starts_with_inline_label(text: &str, settings: SmartMergeSettings) -> bool {
    let text = text.trim_start();
    let Some((colon_index, _)) = text.char_indices().find(|(_, ch)| matches!(ch, ':' | '：'))
    else {
        return false;
    };
    let label = text[..colon_index].trim();
    let label_len = label.chars().count();

    label_len > 0
        && label_len <= settings.inline_label_max_chars
        && label.split_whitespace().count().max(1) <= 5
        && label.chars().any(char::is_alphanumeric)
        && !label
            .chars()
            .any(|ch| matches!(ch, '.' | ',' | ';' | '。' | '，' | '；'))
}

fn append_ocr_fragment(text: &mut String, fragment: &str) {
    if fragment.is_empty() {
        return;
    }
    if text.is_empty() {
        text.push_str(fragment);
        return;
    }

    if text.ends_with('-')
        && fragment
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        text.pop();
        text.push_str(fragment);
    } else if should_insert_space(text, fragment) {
        text.push(' ');
        text.push_str(fragment);
    } else {
        text.push_str(fragment);
    }
}

fn should_insert_space(left: &str, right: &str) -> bool {
    let Some(left_char) = left.chars().last() else {
        return false;
    };
    let Some(right_char) = right.chars().next() else {
        return false;
    };

    (left_char.is_ascii_alphanumeric() || left_char == ')' || left_char == ']')
        && (right_char.is_ascii_alphanumeric() || right_char == '(' || right_char == '[')
}

fn ends_sentence(text: &str) -> bool {
    text.chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
        .is_some_and(|ch| {
            matches!(
                ch,
                '.' | '!' | '?' | ';' | ':' | '。' | '！' | '？' | '；' | '：'
            )
        })
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let min_x = a.origin.x.min(b.origin.x);
    let min_y = a.origin.y.min(b.origin.y);
    let max_x = a.max_x().max(b.max_x());
    let max_y = a.max_y().max(b.max_y());
    Rect::new(
        Point::new(min_x, min_y),
        Size::new((max_x - min_x).max(0.0), (max_y - min_y).max(0.0)),
    )
}

fn vertical_overlap_ratio(a: Rect, b: Rect) -> f32 {
    let overlap = a.max_y().min(b.max_y()) - a.origin.y.max(b.origin.y);
    if overlap <= 0.0 {
        return 0.0;
    }
    overlap / a.size.height.min(b.size.height).max(1.0)
}

fn ocr_text_right_limit_x(
    result: &OcrResult,
    block_index: usize,
    image_rect: EguiRect,
    image_size: Size,
) -> f32 {
    let Some(block) = result.blocks.get(block_index) else {
        return image_rect.max.x;
    };
    let block_rect = ocr_bounds_to_screen_rect(image_rect, image_size, block.bounds);

    result
        .blocks
        .iter()
        .enumerate()
        .filter(|(index, other)| {
            *index != block_index
                && !other.text.trim().is_empty()
                && ocr_bounds_to_screen_rect(image_rect, image_size, other.bounds).left()
                    >= block_rect.right() - 1.0
        })
        .filter_map(|(_, other)| {
            let other_rect = ocr_bounds_to_screen_rect(image_rect, image_size, other.bounds);
            (screen_vertical_overlap_ratio(block_rect, other_rect) >= 0.25)
                .then_some(other_rect.left())
        })
        .fold(image_rect.max.x, f32::min)
        .clamp(block_rect.left() + 1.0, image_rect.max.x)
}

fn ocr_bounds_to_screen_rect(image_rect: EguiRect, image_size: Size, bounds: Rect) -> EguiRect {
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

fn screen_vertical_overlap_ratio(a: EguiRect, b: EguiRect) -> f32 {
    let overlap = a.max.y.min(b.max.y) - a.min.y.max(b.min.y);
    if overlap <= 0.0 {
        return 0.0;
    }

    overlap / a.height().min(b.height()).max(1.0)
}

#[cfg(test)]
mod tests {
    use shared_models::{ImageId, OcrResult, OcrTextBlock};

    use super::*;

    #[test]
    fn smart_merge_separates_title_summary_and_inline_label_rows() {
        let result = ocr_result(vec![
            block("Scenes", 0.0, 0.0, 64.0, 20.0),
            block(
                "Create, save, and load ECS worlds using Bevy's Scene system",
                0.0,
                30.0,
                470.0,
                18.0,
            ),
            block(
                "Loading: Loading scenes preserves entity IDs (useful for save games)",
                0.0,
                58.0,
                520.0,
                18.0,
            ),
            block(
                "Instancing: Instancing creates linked duplicates of scenes with new entity IDs",
                0.0,
                86.0,
                560.0,
                18.0,
            ),
            block(
                "Hot Reloading: Changes to scene files are automatically applied to running apps",
                0.0,
                114.0,
                590.0,
                18.0,
            ),
        ]);

        let groups = translatable_blocks_by_smart_merge(&result, SmartMergeSettings::default());

        assert_eq!(
            group_texts(&groups),
            vec![
                "Scenes",
                "Create, save, and load ECS worlds using Bevy's Scene system",
                "Loading: Loading scenes preserves entity IDs (useful for save games)",
                "Instancing: Instancing creates linked duplicates of scenes with new entity IDs",
                "Hot Reloading: Changes to scene files are automatically applied to running apps",
            ]
        );
    }

    #[test]
    fn smart_merge_keeps_left_aligned_paragraph_with_short_final_line() {
        let result = ocr_result(vec![
            block(
                "This paragraph wraps across multiple OCR",
                12.0,
                0.0,
                330.0,
                18.0,
            ),
            block(
                "lines with the same left edge and similar",
                12.0,
                26.0,
                326.0,
                18.0,
            ),
            block("line height", 12.0, 52.0, 92.0, 18.0),
            block("Next paragraph starts here", 12.0, 82.0, 230.0, 18.0),
        ]);

        let groups = translatable_blocks_by_smart_merge(&result, SmartMergeSettings::default());

        assert_eq!(
            group_texts(&groups),
            vec![
                "This paragraph wraps across multiple OCR lines with the same left edge and similar line height",
                "Next paragraph starts here",
            ]
        );
    }

    #[test]
    fn smart_merge_accepts_right_aligned_wrapped_lines() {
        let result = ocr_result(vec![
            block(
                "Right aligned text can wrap across",
                130.0,
                0.0,
                270.0,
                18.0,
            ),
            block("multiple lines in a side panel", 160.0, 26.0, 240.0, 18.0),
        ]);

        let groups = translatable_blocks_by_smart_merge(&result, SmartMergeSettings::default());

        assert_eq!(
            group_texts(&groups),
            vec!["Right aligned text can wrap across multiple lines in a side panel"]
        );
    }

    #[test]
    fn smart_merge_rejects_height_and_edge_mismatches() {
        let height_mismatch = ocr_result(vec![
            block("Small heading", 20.0, 0.0, 110.0, 14.0),
            block("Large body text", 20.0, 26.0, 160.0, 28.0),
        ]);
        let edge_mismatch = ocr_result(vec![
            block("Left column text", 20.0, 0.0, 150.0, 18.0),
            block("Offset next line", 90.0, 26.0, 190.0, 18.0),
        ]);

        assert_eq!(
            translatable_blocks_by_smart_merge(&height_mismatch, SmartMergeSettings::default())
                .len(),
            2
        );
        assert_eq!(
            translatable_blocks_by_smart_merge(&edge_mismatch, SmartMergeSettings::default()).len(),
            2
        );
    }

    fn group_texts(groups: &[PinTranslatableBlock]) -> Vec<&str> {
        groups.iter().map(|group| group.text.as_str()).collect()
    }

    fn ocr_result(blocks: Vec<OcrTextBlock>) -> OcrResult {
        let plain_text = blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        OcrResult {
            job_id: "test-job".to_owned(),
            image_id: ImageId::new("test-image"),
            blocks,
            plain_text,
        }
    }

    fn block(text: &str, x: f32, y: f32, width: f32, height: f32) -> OcrTextBlock {
        OcrTextBlock {
            text: text.to_owned(),
            bounds: Rect::new(Point::new(x, y), Size::new(width, height)),
            confidence: None,
            language: Some("en".to_owned()),
        }
    }
}

impl App for PinWindowApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.drain_ocr_result(ctx);
        self.drain_translation_result(ctx);
        self.handle_shortcuts(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let canvas = ui.max_rect();
                let response = ui.interact(canvas, Id::new("pin-drag"), Sense::click_and_drag());
                self.sync_display_size_to_canvas(canvas);
                let image_rect = self.image_rect(canvas);
                self.handle_canvas_response(ctx, &response, canvas, image_rect);

                if let Some(texture) = &self.texture {
                    let tint = Color32::from_white_alpha((self.opacity * 255.0).round() as u8);
                    ui.painter().image(
                        texture.id(),
                        image_rect,
                        EguiRect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        tint,
                    );
                    draw_pin_border(ui.painter(), image_rect);
                    self.draw_ocr_overlays(ui.painter(), image_rect);
                    self.draw_toolbar(ui.painter(), canvas, image_rect);
                } else if let Some(error) = &self.error {
                    draw_error(ui.painter(), image_rect, error);
                    self.draw_toolbar(ui.painter(), canvas, image_rect);
                }

                if let Some(status) = &self.ocr_status {
                    draw_pin_status(ui.painter(), canvas, status);
                }
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
