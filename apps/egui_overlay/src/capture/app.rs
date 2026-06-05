use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, Context, FontId, Id, Key, Painter, Pos2, Rect as EguiRect, Sense, Vec2,
    ViewportCommand, WindowLevel,
};
use eframe::{App, CreationContext, Frame};
use image::{DynamicImage, GenericImageView};
use raw_window_handle::HasWindowHandle;
use shared_models::{Point, Rect, Size};

use crate::capture::geometry::{
    CaptureDragMode, CaptureDragState, apply_drag, clamp_pos, selection_drag_mode,
};
use crate::capture::hotkeys::{command_shortcut_pressed, copy_shortcut_pressed, hotkey_pressed};
use crate::capture::paint::{
    draw_error, draw_hint, draw_magnifier, draw_selection_mask, draw_size_label, draw_toolbar,
    format_color_value, snapshot_color_at, toolbar_action_at,
};
use crate::capture::snapshot_io::{
    CaptureRegion, SAVE_CANCELED_CODE, SnapshotTile, build_capture_regions,
    copy_snapshot_to_clipboard, crop_snapshot_to_file, load_shared_snapshot, load_snapshot,
    save_snapshot_to_file,
};
use crate::capture::window::{
    hwnd_from_raw_window_handle, park_resident_window, request_resident_idle_repaint,
    show_capture_window,
};
use crate::overlay::state::OverlayApp;
use crate::pin::launch::{PinWindowLaunch, spawn_pin_window};
use crate::pin::window::PIN_MIN_OPACITY;
use crate::runtime::cli::{CliArgs, OverlayLanguage, parse_color};
use crate::runtime::control::{
    OverlayCaptureCommand, OverlayCommand, OverlayCommandQueue, new_overlay_command_queue,
    start_control_server,
};
use crate::runtime::fonts::install_system_fonts;
use crate::runtime::text::OverlayText;

const SECONDARY_DISMISS_GRACE_MS: u64 = 180;
const CLICK_CAPTURE_MAX_DRAG_DISTANCE: f32 = 3.0;
const MIN_SELECTION_SIZE: f32 = 4.0;
const DEFERRED_SAVE_DELAY_MS: u64 = 80;
pub(crate) struct CaptureOverlayApp {
    state: OverlayApp,
    text: OverlayText,
    screen_origin: Point,
    mask_opacity: f32,
    border_color: Color32,
    show_size_label: bool,
    show_toolbar: bool,
    show_magnifier: bool,
    magnifier_scale: f32,
    pin_hotkey: String,
    completion_action: CaptureAction,
    pin_opacity: f32,
    pin_zoom_step: f32,
    pin_min_width: f32,
    pin_min_height: f32,
    pin_always_on_top: bool,
    ocr_provider: String,
    ocr_language_hint: Option<String>,
    ocr_default_model_id: Option<String>,
    ocr_models_registry: Option<PathBuf>,
    translate_provider: String,
    translate_target_language: String,
    translate_segmentation_mode: String,
    translate_default_model_id: Option<String>,
    ocr_text_font_height_ratio: f32,
    ocr_text_min_font_size: f32,
    ocr_text_max_font_size: f32,
    ocr_text_padding_x: f32,
    ocr_text_padding_y: f32,
    ocr_text_interaction_padding_x: f32,
    ocr_text_interaction_padding_y: f32,
    resident: bool,
    command_queue: Option<OverlayCommandQueue>,
    snapshot_path: Option<PathBuf>,
    snapshot_image: Option<DynamicImage>,
    snapshot_tiles: Vec<SnapshotTile>,
    capture_regions: Vec<CaptureRegion>,
    hovered_region: Option<EguiRect>,
    selection: Option<EguiRect>,
    window_hwnd: Option<isize>,
    drag_state: Option<CaptureDragState>,
    pending_save: Option<PendingSave>,
    status: CaptureStatus,
    color_format: ColorValueFormat,
    shift_down_last_frame: bool,
    last_error: Option<String>,
    created_at: Instant,
}

impl CaptureOverlayApp {
    pub(crate) fn new(creation_context: &CreationContext<'_>, args: CliArgs) -> Self {
        install_system_fonts(&creation_context.egui_ctx);
        let text = OverlayText::new(args.language);
        let command_queue = args.resident.then(|| {
            let queue = new_overlay_command_queue();
            start_control_server(
                args.control_port,
                queue.clone(),
                creation_context.egui_ctx.clone(),
            );
            queue
        });
        let (snapshot_image, snapshot_tiles, last_error) = if args.resident {
            (None, Vec::new(), None)
        } else {
            load_snapshot(&creation_context.egui_ctx, args.snapshot.as_ref(), text)
        };

        Self {
            state: OverlayApp::default(),
            text,
            screen_origin: Point::new(args.x, args.y),
            mask_opacity: args.mask_opacity,
            border_color: args.border_color,
            show_size_label: args.show_size_label,
            show_toolbar: args.show_toolbar,
            show_magnifier: args.show_magnifier,
            magnifier_scale: args.magnifier_scale,
            pin_hotkey: args.pin_hotkey,
            completion_action: CaptureAction::from_name(&args.completion_action),
            pin_opacity: args.pin_opacity,
            pin_zoom_step: args.pin_zoom_step,
            pin_min_width: args.pin_min_width,
            pin_min_height: args.pin_min_height,
            pin_always_on_top: args.pin_always_on_top,
            ocr_provider: args.ocr_provider,
            ocr_language_hint: args.ocr_language_hint,
            ocr_default_model_id: args.ocr_default_model_id,
            ocr_models_registry: args.ocr_models_registry,
            translate_provider: args.translate_provider,
            translate_target_language: args.translate_target_language,
            translate_segmentation_mode: args.translate_segmentation_mode,
            translate_default_model_id: args.translate_default_model_id,
            ocr_text_font_height_ratio: args.ocr_text_font_height_ratio,
            ocr_text_min_font_size: args.ocr_text_min_font_size,
            ocr_text_max_font_size: args.ocr_text_max_font_size,
            ocr_text_padding_x: args.ocr_text_padding_x,
            ocr_text_padding_y: args.ocr_text_padding_y,
            ocr_text_interaction_padding_x: args.ocr_text_interaction_padding_x,
            ocr_text_interaction_padding_y: args.ocr_text_interaction_padding_y,
            resident: args.resident,
            command_queue,
            snapshot_path: args.snapshot,
            snapshot_image,
            snapshot_tiles,
            capture_regions: Vec::new(),
            hovered_region: None,
            selection: None,
            window_hwnd: creation_context
                .window_handle()
                .ok()
                .and_then(|handle| hwnd_from_raw_window_handle(handle.as_raw())),
            drag_state: None,
            pending_save: None,
            status: if args.resident {
                CaptureStatus::Idle
            } else {
                CaptureStatus::Selecting
            },
            color_format: ColorValueFormat::Hex,
            shift_down_last_frame: false,
            last_error,
            created_at: Instant::now(),
        }
    }

    fn update_color_format_toggle(&mut self, ctx: &Context) {
        let shift_down = ctx.input(|input| input.modifiers.shift);
        if shift_down && !self.shift_down_last_frame {
            self.color_format = self.color_format.toggled();
        }
        self.shift_down_last_frame = shift_down;
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.input(|input| input.key_pressed(Key::Escape)) {
            self.dismiss(ctx);
            return;
        }

        if ctx.input(|input| hotkey_pressed(input, &self.pin_hotkey)) {
            self.run_capture_action(ctx, CaptureAction::Pin);
            return;
        }

        if ctx.input(copy_shortcut_pressed) {
            self.run_capture_action(ctx, CaptureAction::Copy);
            return;
        }

        if ctx.input(|input| command_shortcut_pressed(input, Key::S)) {
            self.run_capture_action(ctx, CaptureAction::Save);
            return;
        }

        if ctx.input(|input| input.key_pressed(Key::Enter)) {
            self.finish_selection(ctx);
            return;
        }

        if ctx.input(|input| input.key_pressed(Key::P)) {
            self.run_capture_action(ctx, CaptureAction::Pin);
        }
    }

    fn handle_color_shortcuts(&mut self, ctx: &Context, canvas: EguiRect) {
        if ctx.input(|input| {
            input.key_pressed(Key::C)
                && !input.modifiers.ctrl
                && !input.modifiers.command
                && !input.modifiers.alt
        }) {
            self.copy_hovered_color(ctx, canvas);
        }
    }

    fn handle_pointer(&mut self, ctx: &Context, canvas: EguiRect) -> Option<CaptureAction> {
        let pointer = ctx.input(|input| input.pointer.clone());

        if self.drag_state.is_none() && self.selection.is_none() {
            self.hovered_region = pointer
                .interact_pos()
                .map(|position| clamp_pos(position, canvas))
                .and_then(|position| self.region_at(position));
        } else if self.selection.is_some() {
            self.hovered_region = None;
        }

        if pointer.primary_pressed() {
            if let Some(position) = pointer.interact_pos() {
                let clamped = clamp_pos(position, canvas);
                if let Some(selection) = self.selection {
                    if self.show_toolbar
                        && let Some(action) =
                            toolbar_action_at(clamped, canvas, selection, self.text)
                    {
                        return Some(action);
                    }

                    let mode = selection_drag_mode(selection, clamped);
                    self.drag_state = Some(CaptureDragState {
                        start: clamped,
                        original: selection,
                        mode,
                    });
                } else {
                    self.drag_state = Some(CaptureDragState {
                        start: clamped,
                        original: EguiRect::from_two_pos(clamped, clamped),
                        mode: CaptureDragMode::Create,
                    });
                    self.selection = Some(EguiRect::from_two_pos(clamped, clamped));
                }
            }
        }

        if pointer.primary_down() {
            if let (Some(drag), Some(position)) = (self.drag_state, pointer.interact_pos()) {
                let clamped = clamp_pos(position, canvas);
                self.selection = Some(apply_drag(drag, clamped, canvas));
            }
        }

        if pointer.primary_released() {
            let released_at = pointer
                .interact_pos()
                .map(|position| clamp_pos(position, canvas));
            let was_click = match (self.drag_state, released_at) {
                (Some(drag), Some(end)) => {
                    drag.start.distance(end) <= CLICK_CAPTURE_MAX_DRAG_DISTANCE
                }
                _ => false,
            };
            let clicked_region = released_at.and_then(|position| self.region_at(position));
            let drag_mode = self.drag_state.map(|drag| drag.mode);
            self.drag_state = None;

            if was_click && matches!(drag_mode, Some(CaptureDragMode::Create)) {
                if let Some(region) = clicked_region.or(self.hovered_region) {
                    self.selection = Some(region);
                } else {
                    self.selection = None;
                }
            } else if let Some(selection) = self.selection {
                if selection.width() < MIN_SELECTION_SIZE || selection.height() < MIN_SELECTION_SIZE
                {
                    self.selection = None;
                }
            }
        }

        if pointer.secondary_clicked()
            && self.created_at.elapsed() > Duration::from_millis(SECONDARY_DISMISS_GRACE_MS)
        {
            self.dismiss(ctx);
        }

        None
    }

    fn draw(&self, painter: &Painter, canvas: EguiRect, ctx: &Context) {
        let mask_alpha = (self.mask_opacity * 255.0).round() as u8;
        self.draw_snapshot(painter, canvas);
        let hovered_pixel = self.hovered_pixel(ctx, canvas);

        if let Some(selection) = self.selection {
            draw_selection_mask(painter, canvas, selection, mask_alpha, self.border_color);
            if self.show_size_label {
                draw_size_label(painter, selection);
            }
            if self.show_toolbar {
                draw_toolbar(painter, canvas, selection, self.border_color, self.text);
            }
        } else if let Some(region) = self.hovered_region {
            draw_selection_mask(painter, canvas, region, mask_alpha, self.border_color);
            if self.show_size_label {
                draw_size_label(painter, region);
            }
        } else if self.created_at.elapsed() > Duration::from_millis(160) {
            painter.rect_filled(canvas, 0.0, Color32::from_black_alpha(mask_alpha));
            draw_hint(painter, canvas, self.text.drag_hint);
        } else {
            painter.rect_filled(canvas, 0.0, Color32::from_black_alpha(mask_alpha));
        }

        if let Some(error) = &self.last_error {
            draw_error(painter, canvas, error);
        }

        if self.show_magnifier {
            if let Some(pixel) = hovered_pixel.filter(|pixel| {
                self.selection
                    .is_none_or(|selection| selection.contains(pixel.position))
            }) {
                draw_magnifier(
                    painter,
                    canvas,
                    &self.snapshot_image,
                    pixel,
                    self.magnifier_scale,
                    self.color_format,
                );
            }
        }

        if matches!(self.status, CaptureStatus::Capturing) {
            painter.rect_filled(canvas, 0.0, Color32::from_black_alpha(28));
            painter.text(
                canvas.center(),
                Align2::CENTER_CENTER,
                self.text.capturing,
                FontId::proportional(16.0),
                Color32::WHITE,
            );
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn draw_snapshot(&self, painter: &Painter, canvas: EguiRect) {
        if !self.snapshot_tiles.is_empty() {
            for tile in &self.snapshot_tiles {
                painter.image(
                    tile.texture.id(),
                    tile.rect.translate(canvas.min.to_vec2()),
                    EguiRect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        } else {
            painter.rect_filled(canvas, 0.0, Color32::from_rgb(20, 24, 27));
        }
    }

    fn region_at(&self, position: Pos2) -> Option<EguiRect> {
        self.capture_regions
            .iter()
            .filter(|region| region.rect.contains(position))
            .min_by(|a, b| {
                a.rect
                    .area()
                    .total_cmp(&b.rect.area())
                    .then_with(|| b.depth.cmp(&a.depth))
            })
            .map(|region| region.rect)
    }

    fn hovered_pixel(&self, ctx: &Context, canvas: EguiRect) -> Option<PointerPixel> {
        let position = ctx.input(|input| input.pointer.hover_pos())?;
        if !canvas.contains(position) {
            return None;
        }

        pointer_pixel_at(
            self.snapshot_image.as_ref()?,
            position,
            canvas,
            self.screen_origin,
        )
    }

    fn copy_hovered_color(&mut self, ctx: &Context, canvas: EguiRect) {
        let Some(pixel) = self.hovered_pixel(ctx, canvas) else {
            return;
        };

        let value = format_color_value(pixel.color, self.color_format);
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(value.clone()))
        {
            Ok(()) => {
                log::info!("copied hovered color {value}; dismissing capture overlay");
                self.dismiss(ctx);
            }
            Err(error) => {
                let message = format!("failed to copy color: {error}");
                log::error!("{message}");
                self.last_error = Some(message);
            }
        }
    }

    fn finish_selection(&mut self, ctx: &Context) {
        self.run_capture_action(ctx, self.completion_action);
    }

    fn run_capture_action(&mut self, ctx: &Context, action: CaptureAction) {
        log::info!(
            "capture action requested action={:?} status={:?} selection={:?} hwnd={:?}",
            action,
            self.status,
            self.selection,
            self.window_hwnd
        );
        if matches!(self.status, CaptureStatus::Idle | CaptureStatus::Capturing) {
            log::info!("capture action ignored because status={:?}", self.status);
            return;
        }

        let Some(selection) = self.selection else {
            log::info!("capture action ignored because selection is empty");
            return;
        };

        if selection.width() < MIN_SELECTION_SIZE || selection.height() < MIN_SELECTION_SIZE {
            log::info!(
                "capture action ignored because selection is too small: {}x{}",
                selection.width(),
                selection.height()
            );
            return;
        }

        self.status = CaptureStatus::Capturing;
        let region = self.screen_rect(selection);
        if matches!(action, CaptureAction::Save) {
            self.begin_deferred_save(ctx, selection);
            return;
        }

        let result = match action {
            CaptureAction::Pin => self.capture_selection_to_pin(selection, region),
            CaptureAction::Copy => self.copy_selection_to_clipboard(selection),
            CaptureAction::Editor => {
                log::warn!("capture editor action is not implemented; falling back to pin");
                self.capture_selection_to_pin(selection, region)
            }
            CaptureAction::Save => unreachable!("save is handled as a deferred modal action"),
        };

        match result {
            Ok(()) => {
                log::info!("capture action completed action={:?}", action);
                self.dismiss(ctx);
            }
            Err(error) if error == SAVE_CANCELED_CODE => {
                log::info!("capture action canceled action={:?}", action);
                self.status = CaptureStatus::Selecting;
            }
            Err(error) => {
                log::error!("capture action failed action={:?}: {}", action, error);
                self.status = CaptureStatus::Selecting;
                self.last_error = Some(error);
            }
        }
    }

    fn begin_deferred_save(&mut self, ctx: &Context, selection: EguiRect) {
        log::info!("deferred save scheduled selection={selection:?}");
        self.pending_save = Some(PendingSave {
            selection,
            requested_at: Instant::now(),
        });
        ctx.request_repaint_after(Duration::from_millis(DEFERRED_SAVE_DELAY_MS));
    }

    fn run_pending_save(&mut self, ctx: &Context) {
        let Some(pending) = self.pending_save else {
            return;
        };

        if pending.requested_at.elapsed() < Duration::from_millis(DEFERRED_SAVE_DELAY_MS) {
            ctx.request_repaint_after(Duration::from_millis(DEFERRED_SAVE_DELAY_MS));
            return;
        }

        self.pending_save = None;
        log::info!("opening save dialog hwnd={:?}", self.window_hwnd);
        if let Some(hwnd) = self.window_hwnd {
            platform_win32::suspend_window_for_modal_dialog(hwnd);
        }

        match self.save_selection_to_file(pending.selection) {
            Ok(()) => {
                log::info!("save dialog completed; dismissing capture overlay");
                self.dismiss(ctx);
            }
            Err(error) if error == SAVE_CANCELED_CODE => {
                log::info!("save dialog canceled; restoring capture overlay");
                self.status = CaptureStatus::Selecting;
                self.restore_capture_window(ctx);
            }
            Err(error) => {
                log::error!("save dialog failed; restoring capture overlay: {}", error);
                self.status = CaptureStatus::Selecting;
                self.last_error = Some(error);
                self.restore_capture_window(ctx);
            }
        }
    }

    fn restore_capture_window(&self, ctx: &Context) {
        if let Some(hwnd) = self.window_hwnd {
            platform_win32::restore_window_after_modal_dialog(hwnd, true);
        }

        if let Some(snapshot) = &self.snapshot_image {
            let (width, height) = snapshot.dimensions();
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(
                self.screen_origin.x,
                self.screen_origin.y,
            )));
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
                width as f32,
                height as f32,
            )));
        }

        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        ctx.request_repaint();
    }

    fn capture_selection_to_pin(&self, selection: EguiRect, region: Rect) -> Result<(), String> {
        let Some(snapshot) = &self.snapshot_image else {
            log::error!("pin failed: missing snapshot image");
            return Err(self.text.missing_snapshot.to_owned());
        };

        log::info!(
            "pin capture requested selection={:?} region={:?}",
            selection,
            region
        );
        let cropped = crop_snapshot_to_file(snapshot, selection, &self.text)?;
        log::info!(
            "pin capture cropped path={} size={}x{}",
            cropped.path.display(),
            cropped.width,
            cropped.height
        );
        spawn_pin_window(&PinWindowLaunch {
            image_path: &cropped.path,
            x: region.origin.x,
            y: region.origin.y,
            width: cropped.width as f32,
            height: cropped.height as f32,
            opacity: self.pin_opacity,
            zoom_step: self.pin_zoom_step,
            min_width: self.pin_min_width,
            min_height: self.pin_min_height,
            always_on_top: self.pin_always_on_top,
            ocr_provider: &self.ocr_provider,
            ocr_language_hint: self.ocr_language_hint.as_deref(),
            ocr_default_model_id: self.ocr_default_model_id.as_deref(),
            ocr_models_registry: self.ocr_models_registry.as_deref(),
            translate_provider: &self.translate_provider,
            translate_target_language: &self.translate_target_language,
            translate_segmentation_mode: &self.translate_segmentation_mode,
            translate_default_model_id: self.translate_default_model_id.as_deref(),
            ocr_text_font_height_ratio: self.ocr_text_font_height_ratio,
            ocr_text_min_font_size: self.ocr_text_min_font_size,
            ocr_text_max_font_size: self.ocr_text_max_font_size,
            ocr_text_padding_x: self.ocr_text_padding_x,
            ocr_text_padding_y: self.ocr_text_padding_y,
            ocr_text_interaction_padding_x: self.ocr_text_interaction_padding_x,
            ocr_text_interaction_padding_y: self.ocr_text_interaction_padding_y,
        })
        .map_err(|error| format!("{}: {error}", self.text.pin_spawn_failed))
    }

    fn copy_selection_to_clipboard(&self, selection: EguiRect) -> Result<(), String> {
        let Some(snapshot) = &self.snapshot_image else {
            return Err(self.text.missing_snapshot.to_owned());
        };

        copy_snapshot_to_clipboard(snapshot, selection, &self.text)
    }

    fn save_selection_to_file(&self, selection: EguiRect) -> Result<(), String> {
        let Some(snapshot) = &self.snapshot_image else {
            log::error!("save failed: missing snapshot image");
            return Err(self.text.missing_snapshot.to_owned());
        };

        save_snapshot_to_file(snapshot, selection, &self.text).map(|_| ())
    }

    fn screen_rect(&self, selection: EguiRect) -> Rect {
        Rect::new(
            Point::new(
                self.screen_origin.x + selection.min.x,
                self.screen_origin.y + selection.min.y,
            ),
            Size::new(selection.width(), selection.height()),
        )
    }

    fn dismiss(&mut self, ctx: &Context) {
        log::info!(
            "dismiss capture overlay resident={} hwnd={:?}",
            self.resident,
            self.window_hwnd
        );
        if self.resident {
            self.clear_capture_state();
            self.status = CaptureStatus::Idle;
            park_resident_window(ctx, self.window_hwnd);
            request_resident_idle_repaint(ctx);
        } else {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn clear_capture_state(&mut self) {
        self.snapshot_path = None;
        self.snapshot_image = None;
        self.snapshot_tiles.clear();
        self.capture_regions.clear();
        self.hovered_region = None;
        self.selection = None;
        self.drag_state = None;
        self.pending_save = None;
        self.last_error = None;
        self.created_at = Instant::now();
    }

    fn drain_commands(&mut self, ctx: &Context) {
        let Some(queue) = &self.command_queue else {
            return;
        };

        let commands = match queue.lock() {
            Ok(mut queue) => queue.drain(..).collect::<Vec<_>>(),
            Err(_) => {
                self.last_error = Some("overlay command queue lock poisoned".to_owned());
                return;
            }
        };

        for queued in commands {
            let result = match queued.command {
                OverlayCommand::Capture(command) => self.apply_capture_command(ctx, command),
                OverlayCommand::PinSelection => self.pin_current_selection(ctx),
                OverlayCommand::Error(error) => {
                    self.last_error = Some(error.clone());
                    Err(error)
                }
            };

            if let Some(completion) = queued.completion {
                let _ = completion.send(result);
            }
        }
    }

    fn pin_current_selection(&mut self, ctx: &Context) -> Result<(), String> {
        if !matches!(self.status, CaptureStatus::Selecting) {
            return Err("capture_overlay_inactive".to_owned());
        }

        let Some(selection) = self.selection else {
            return Err("capture_selection_missing".to_owned());
        };

        if selection.width() < MIN_SELECTION_SIZE || selection.height() < MIN_SELECTION_SIZE {
            return Err("capture_selection_too_small".to_owned());
        }

        self.run_capture_action(ctx, CaptureAction::Pin);
        Ok(())
    }

    fn apply_capture_command(
        &mut self,
        ctx: &Context,
        command: OverlayCaptureCommand,
    ) -> Result<(), String> {
        if command.kind != "capture" {
            let error = format!("unknown overlay command: {}", command.kind);
            log::error!("{error}");
            self.last_error = Some(error.clone());
            return Err(error);
        }

        log::info!(
            "capture command received snapshot={}x{} origin=({}, {}) regions={}",
            command.snapshot.width,
            command.snapshot.height,
            command.snapshot.origin_x,
            command.snapshot.origin_y,
            command.regions.len()
        );
        self.pending_save = None;
        self.text = OverlayText::new(OverlayLanguage::from_code(&command.language));
        self.mask_opacity = command.mask_opacity.clamp(0.0, 0.9);
        self.show_size_label = command.show_size_label;
        self.show_toolbar = command.show_toolbar;
        self.show_magnifier = command.show_magnifier;
        self.magnifier_scale = command.magnifier_scale.clamp(1.0, 6.0);
        self.pin_hotkey = command.pin_hotkey;
        self.completion_action = CaptureAction::from_name(&command.completion_action);
        self.pin_opacity = command.pin_opacity.clamp(PIN_MIN_OPACITY, 1.0);
        self.pin_zoom_step = command.pin_zoom_step.clamp(0.05, 0.5);
        self.pin_min_width = command.pin_min_width.clamp(16.0, 2048.0);
        self.pin_min_height = command.pin_min_height.clamp(16.0, 2048.0);
        self.pin_always_on_top = command.pin_always_on_top;
        self.ocr_provider = command.ocr_provider;
        self.ocr_language_hint = empty_to_none(command.ocr_language_hint);
        self.ocr_default_model_id = empty_to_none(command.ocr_default_model_id);
        self.ocr_models_registry = command.ocr_models_registry.and_then(|path| {
            let path = path.trim();
            (!path.is_empty()).then(|| PathBuf::from(path))
        });
        self.translate_provider = command.translate_provider;
        self.translate_target_language = command.translate_target_language;
        self.translate_segmentation_mode = command.translate_segmentation_mode;
        self.translate_default_model_id = empty_to_none(command.translate_default_model_id);
        self.ocr_text_font_height_ratio = command.ocr_text_font_height_ratio.clamp(0.1, 2.0);
        self.ocr_text_min_font_size = command.ocr_text_min_font_size.clamp(4.0, 96.0);
        self.ocr_text_max_font_size = command
            .ocr_text_max_font_size
            .clamp(self.ocr_text_min_font_size, 128.0);
        self.ocr_text_padding_x = command.ocr_text_padding_x.clamp(0.0, 32.0);
        self.ocr_text_padding_y = command.ocr_text_padding_y.clamp(0.0, 32.0);
        self.ocr_text_interaction_padding_x =
            command.ocr_text_interaction_padding_x.clamp(0.0, 48.0);
        self.ocr_text_interaction_padding_y =
            command.ocr_text_interaction_padding_y.clamp(0.0, 48.0);
        if let Some(color) = parse_color(&command.border_color) {
            self.border_color = color;
        }

        match load_shared_snapshot(ctx, &command.snapshot, self.text) {
            Ok(snapshot) => {
                log::info!("capture snapshot loaded");
                self.snapshot_path = None;
                self.snapshot_image = Some(snapshot.image);
                self.snapshot_tiles = snapshot.tiles;
                self.capture_regions = build_capture_regions(&command.regions, &command.snapshot);
                self.hovered_region = None;
                self.screen_origin =
                    Point::new(command.snapshot.origin_x, command.snapshot.origin_y);
                self.selection = None;
                self.drag_state = None;
                self.status = CaptureStatus::Selecting;
                self.color_format = ColorValueFormat::Hex;
                self.shift_down_last_frame = false;
                self.last_error = None;
                self.created_at = Instant::now();
                show_capture_window(ctx, &command.snapshot);
                Ok(())
            }
            Err(error) => {
                log::error!("capture snapshot load failed: {error}");
                self.clear_capture_state();
                self.status = CaptureStatus::Selecting;
                self.capture_regions = build_capture_regions(&command.regions, &command.snapshot);
                self.screen_origin =
                    Point::new(command.snapshot.origin_x, command.snapshot.origin_y);
                self.last_error = Some(error);
                show_capture_window(ctx, &command.snapshot);
                Ok(())
            }
        }
    }
}

impl App for CaptureOverlayApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.drain_commands(ctx);
        self.run_pending_save(ctx);
        if matches!(self.status, CaptureStatus::Idle) {
            if self.resident {
                request_resident_idle_repaint(ctx);
            }
            return;
        }

        self.handle_shortcuts(ctx);
        self.update_color_format_toggle(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let canvas = ui.max_rect();
                let response = ui.interact(canvas, Id::new("capture-canvas"), Sense::drag());
                if response.double_clicked() {
                    self.finish_selection(ctx);
                }

                self.handle_color_shortcuts(ctx, canvas);
                if let Some(action) = self.handle_pointer(ctx, canvas) {
                    self.run_capture_action(ctx, action);
                }
                self.draw(ui.painter(), canvas, ctx);
            });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureStatus {
    Idle,
    Selecting,
    Capturing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureAction {
    Pin,
    Copy,
    Save,
    Editor,
}

impl CaptureAction {
    fn from_name(value: &str) -> Self {
        match value {
            "copy" => Self::Copy,
            "save" => Self::Save,
            "editor" => Self::Editor,
            _ => Self::Pin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorValueFormat {
    Hex,
    Rgb,
}

impl ColorValueFormat {
    fn toggled(self) -> Self {
        match self {
            Self::Hex => Self::Rgb,
            Self::Rgb => Self::Hex,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PendingSave {
    selection: EguiRect,
    requested_at: Instant,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PointerPixel {
    pub(crate) position: Pos2,
    pub(crate) image_x: u32,
    pub(crate) image_y: u32,
    pub(crate) screen_x: i32,
    pub(crate) screen_y: i32,
    pub(crate) color: Color32,
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn pointer_pixel_at(
    snapshot: &DynamicImage,
    position: Pos2,
    canvas: EguiRect,
    screen_origin: Point,
) -> Option<PointerPixel> {
    let (width, height) = snapshot.dimensions();
    if width == 0 || height == 0 || canvas.width() <= 0.0 || canvas.height() <= 0.0 {
        return None;
    }

    let local_x = ((position.x - canvas.min.x) / canvas.width()).clamp(0.0, 1.0);
    let local_y = ((position.y - canvas.min.y) / canvas.height()).clamp(0.0, 1.0);
    let image_x = (local_x * (width.saturating_sub(1) as f32)).round() as u32;
    let image_y = (local_y * (height.saturating_sub(1) as f32)).round() as u32;
    let color = snapshot_color_at(snapshot, image_x, image_y);

    Some(PointerPixel {
        position,
        image_x,
        image_y,
        screen_x: (screen_origin.x + image_x as f32).round() as i32,
        screen_y: (screen_origin.y + image_y as f32).round() as i32,
        color,
    })
}
