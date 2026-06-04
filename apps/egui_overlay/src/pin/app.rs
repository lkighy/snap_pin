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
use shared_models::{OcrProvider, OcrResult, Size, TextOverlay};

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
        if self.toolbar_edge.is_some() {
            let canvas = EguiRect::from_min_size(
                Pos2::ZERO,
                viewport_rect.map_or(self.image_display_size, |rect| rect.size()),
            );
            self.hide_toolbar(ctx, canvas);
            return;
        }

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

        let image_rect = self.image_rect(EguiRect::from_min_size(Pos2::ZERO, viewport_rect.size()));
        let anchor = pointer_pos.unwrap_or(image_rect.center());
        let anchor_fraction = Vec2::new(
            ((anchor.x - image_rect.min.x) / current_size.x).clamp(0.0, 1.0),
            ((anchor.y - image_rect.min.y) / current_size.y).clamp(0.0, 1.0),
        );
        let offset = Vec2::new(
            (current_size.x - new_size.x) * anchor_fraction.x,
            (current_size.y - new_size.y) * anchor_fraction.y,
        );

        self.image_display_size = new_size;
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(viewport_rect.min + offset));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(new_size));
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
            self.run_toolbar_action(ctx, canvas, action);
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
            self.hide_toolbar(ctx, canvas);
            ctx.request_repaint();
            return;
        }

        if response.clicked_by(PointerButton::Primary) && !pointer_over_toolbar {
            self.selected_ocr_block = None;
            self.hide_toolbar(ctx, canvas);
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

    fn run_toolbar_action(&mut self, ctx: &Context, canvas: EguiRect, action: PinToolbarAction) {
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
                log::info!(
                    "pin toolbar requested translation image={:?}; core-service IPC hook pending",
                    self.image_path
                );
            }
            PinToolbarAction::Close => {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }

        if !matches!(
            action,
            PinToolbarAction::Close
                | PinToolbarAction::CopySelectedText
                | PinToolbarAction::CopyAllText
                | PinToolbarAction::RunOcr
                | PinToolbarAction::CloseOcr
        ) {
            self.hide_toolbar(ctx, canvas);
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
            let overlay = TextOverlay {
                text: block.text.clone(),
                language: block.language.clone(),
                bounds: block.bounds,
                role: shared_models::TextOverlayRole::Ocr,
                confidence: block.confidence,
            };
            draw_text_overlay(
                painter,
                image_rect,
                image_size,
                &overlay,
                self.ocr_text_style,
                self.selected_ocr_block == Some(index),
            );
        }
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

    fn close_ocr(&mut self, ctx: &Context) {
        let previous_edge = self.toolbar_edge;
        self.ocr_receiver = None;
        self.ocr_result = None;
        self.ocr_status = None;
        self.selected_ocr_block = None;
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
                self.ocr_result = Some(result);
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
        let result = self.ocr_result.as_ref()?;
        let text = result.plain_text.trim();
        (!text.is_empty()).then(|| text.to_owned())
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
        let image_path = match platform_win32::prompt_save_png_path(&default_name) {
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

impl App for PinWindowApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.drain_ocr_result(ctx);
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
