#![allow(dead_code)]

mod capture;
mod overlay;
mod pin;
mod runtime;

use std::borrow::Cow;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Sender},
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, Align2, Color32, ColorImage, Context, CornerRadius, FontId, Id, Key, Painter,
    PointerButton, Pos2, Rect as EguiRect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2, ViewportBuilder, ViewportCommand, WindowLevel,
};
use eframe::{App, CreationContext, Frame, NativeOptions};
use image::{DynamicImage, GenericImageView};
use model_registry::ModelRegistry;
use ocr_engine::{OcrEngine, RoutedOcrEngine};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::Deserialize;
use shared_models::{
    ImageData, ImageFormat, ImageId, ImageMetadata, OcrExternalProvider, OcrJob, OcrLocalBackend,
    OcrProvider, OcrResult, Point, Rect, Size, TextOverlay,
};

use capture::hotkeys::{
    command_shift_shortcut_pressed, command_shortcut_pressed, copy_shortcut_pressed, hotkey_pressed,
};
use capture::paint::{
    draw_error, draw_hint, draw_magnifier, draw_pin_border, draw_selection_mask, draw_size_label,
    draw_toolbar, format_color_value, snapshot_color_at, toolbar_action_at,
};
use overlay::state::OverlayApp;
use runtime::cli::{CliArgs, OverlayLanguage, OverlayRunMode, parse_color};
use runtime::fonts::install_system_fonts;
use runtime::logging::init_logging;
use runtime::text::OverlayText;

const MIN_SELECTION_SIZE: f32 = 4.0;
const CONTROL_PROTOCOL_VERSION: u32 = 2;
const COMMAND_ACK_TIMEOUT_MS: u64 = 5_000;
const RESIDENT_IDLE_SIZE: f32 = 1.0;
const RESIDENT_IDLE_X: f32 = -32000.0;
const RESIDENT_IDLE_Y: f32 = -32000.0;
const RESIDENT_IDLE_REPAINT_MS: u64 = 100;
const SECONDARY_DISMISS_GRACE_MS: u64 = 180;
const CLICK_CAPTURE_MAX_DRAG_DISTANCE: f32 = 3.0;
const SELECTION_EDGE_HIT_SIZE: f32 = 8.0;
const SELECTION_MIN_SIZE: f32 = 12.0;
const DEFERRED_SAVE_DELAY_MS: u64 = 80;
const SAVE_CANCELED_CODE: &str = "save_canceled";
const MAGNIFIER_SAMPLE_SIZE: i32 = 17;
const PIN_OPACITY_STEP: f32 = 0.05;
const PIN_MIN_OPACITY: f32 = 0.2;
const PIN_MIN_WIDTH: f32 = 96.0;
const PIN_MIN_HEIGHT: f32 = 72.0;
const PIN_MAX_SIDE: f32 = 8192.0;
const PIN_TOOLBAR_BUTTON_SIZE: f32 = 30.0;
const PIN_TOOLBAR_BUTTON_GAP: f32 = 6.0;
const PIN_TOOLBAR_MAX_BUTTONS: usize = 7;
const PIN_TOOLBAR_PADDING: f32 = 6.0;
const PIN_TOOLBAR_MARGIN: f32 = 8.0;
const PIN_TOOLBAR_OUTSIDE_GAP: f32 = 4.0;
const OCR_TEXT_OVERLAY_PADDING_X: f32 = 2.0;
const OCR_TEXT_OVERLAY_PADDING_Y: f32 = 1.0;
const OCR_TEXT_INTERACTION_PADDING_X: f32 = 4.0;
const OCR_TEXT_INTERACTION_PADDING_Y: f32 = 6.0;
const OCR_TEXT_FONT_HEIGHT_RATIO: f32 = 0.54;
const OCR_TEXT_FONT_MIN_SIZE: f32 = 7.0;
const OCR_TEXT_FONT_MAX_SIZE: f32 = 42.0;

fn main() -> eframe::Result<()> {
    init_logging();
    let args = CliArgs::parse();
    log::info!(
        "overlay starting mode={:?} resident={} snapshot={:?} image={:?}",
        args.mode,
        args.resident,
        args.snapshot,
        args.image
    );
    if matches!(args.mode, OverlayRunMode::Capture) && !args.resident && args.snapshot.is_none() {
        log::error!("capture overlay missing --snapshot in non-resident mode");
        eprintln!("snap pin capture overlay requires --snapshot <path> or --resident");
        return Ok(());
    }

    let options = native_options(&args);
    let title_text = OverlayText::new(args.language);
    let title = match args.mode {
        OverlayRunMode::Capture => title_text.capture_title,
        OverlayRunMode::Pin => "snap pin",
    };

    eframe::run_native(
        title,
        options,
        Box::new(move |creation_context| match args.mode {
            OverlayRunMode::Capture => Ok(Box::new(CaptureOverlayApp::new(creation_context, args))),
            OverlayRunMode::Pin => Ok(Box::new(PinWindowApp::new(creation_context, args))),
        }),
    )
}

fn native_options(args: &CliArgs) -> NativeOptions {
    let viewport = match args.mode {
        OverlayRunMode::Capture => {
            let viewport = ViewportBuilder::default()
                .with_decorations(false)
                .with_transparent(false)
                .with_always_on_top()
                .with_resizable(false)
                .with_taskbar(false);

            if args.resident {
                viewport
                    .with_position([RESIDENT_IDLE_X, RESIDENT_IDLE_Y])
                    .with_inner_size([RESIDENT_IDLE_SIZE, RESIDENT_IDLE_SIZE])
                    .with_transparent(true)
                    .with_visible(true)
            } else {
                viewport
                    .with_position([args.x, args.y])
                    .with_inner_size([args.width, args.height])
                    .with_visible(true)
            }
        }
        OverlayRunMode::Pin => {
            let level = if args.pin_always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            };
            let pin_size = clamp_pin_window_size(Vec2::new(args.width, args.height));
            ViewportBuilder::default()
                .with_decorations(false)
                .with_transparent(true)
                .with_window_level(level)
                .with_resizable(true)
                .with_taskbar(false)
                .with_position([args.x, args.y])
                .with_inner_size([pin_size.x, pin_size.y])
                .with_min_inner_size([96.0, 72.0])
        }
    };

    NativeOptions {
        viewport,
        ..Default::default()
    }
}

struct CaptureOverlayApp {
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
    pin_always_on_top: bool,
    ocr_provider: String,
    ocr_language_hint: Option<String>,
    ocr_default_model_id: Option<String>,
    ocr_models_registry: Option<PathBuf>,
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
    fn new(creation_context: &CreationContext<'_>, args: CliArgs) -> Self {
        install_system_fonts(&creation_context.egui_ctx);
        let text = OverlayText::new(args.language);
        let command_queue = args.resident.then(|| {
            let queue = Arc::new(Mutex::new(VecDeque::new()));
            start_control_server(
                args.control_port,
                Arc::clone(&queue),
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
            pin_always_on_top: args.pin_always_on_top,
            ocr_provider: args.ocr_provider,
            ocr_language_hint: args.ocr_language_hint,
            ocr_default_model_id: args.ocr_default_model_id,
            ocr_models_registry: args.ocr_models_registry,
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
            always_on_top: self.pin_always_on_top,
            ocr_provider: &self.ocr_provider,
            ocr_language_hint: self.ocr_language_hint.as_deref(),
            ocr_default_model_id: self.ocr_default_model_id.as_deref(),
            ocr_models_registry: self.ocr_models_registry.as_deref(),
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
        self.pin_always_on_top = command.pin_always_on_top;
        self.ocr_provider = command.ocr_provider;
        self.ocr_language_hint = empty_to_none(command.ocr_language_hint);
        self.ocr_default_model_id = empty_to_none(command.ocr_default_model_id);
        self.ocr_models_registry = command.ocr_models_registry.and_then(|path| {
            let path = path.trim();
            (!path.is_empty()).then(|| PathBuf::from(path))
        });
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
enum CaptureAction {
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
enum ColorValueFormat {
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
struct CaptureDragState {
    start: Pos2,
    original: EguiRect,
    mode: CaptureDragMode,
}

#[derive(Debug, Clone, Copy)]
struct PendingSave {
    selection: EguiRect,
    requested_at: Instant,
}

#[derive(Debug, Clone, Copy)]
struct PointerPixel {
    position: Pos2,
    image_x: u32,
    image_y: u32,
    screen_x: i32,
    screen_y: i32,
    color: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureDragMode {
    Create,
    Move,
    Resize(ResizeEdges),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ResizeEdges {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

#[derive(Debug)]
enum OverlayCommand {
    Capture(OverlayCaptureCommand),
    PinSelection,
    Error(String),
}

type OverlayCommandQueue = Arc<Mutex<VecDeque<QueuedOverlayCommand>>>;

#[derive(Debug)]
struct QueuedOverlayCommand {
    command: OverlayCommand,
    completion: Option<Sender<Result<(), String>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayCaptureCommand {
    kind: String,
    snapshot: SharedSnapshotCommand,
    regions: Vec<CaptureRegionCommand>,
    language: String,
    mask_opacity: f32,
    border_color: String,
    #[serde(default = "default_true")]
    show_size_label: bool,
    #[serde(default = "default_true")]
    show_toolbar: bool,
    #[serde(default = "default_show_magnifier")]
    show_magnifier: bool,
    #[serde(default = "default_magnifier_scale")]
    magnifier_scale: f32,
    #[serde(default = "default_pin_hotkey")]
    pin_hotkey: String,
    #[serde(default = "default_completion_action")]
    completion_action: String,
    #[serde(default = "default_pin_opacity")]
    pin_opacity: f32,
    #[serde(default = "default_pin_zoom_step")]
    pin_zoom_step: f32,
    #[serde(default = "default_true")]
    pin_always_on_top: bool,
    #[serde(default = "default_ocr_provider")]
    ocr_provider: String,
    #[serde(default)]
    ocr_language_hint: Option<String>,
    #[serde(default)]
    ocr_default_model_id: Option<String>,
    #[serde(default)]
    ocr_models_registry: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_show_magnifier() -> bool {
    true
}

fn default_magnifier_scale() -> f32 {
    2.0
}

fn default_pin_hotkey() -> String {
    "Ctrl+Shift+X".to_owned()
}

fn default_completion_action() -> String {
    "pin".to_owned()
}

fn default_pin_opacity() -> f32 {
    1.0
}

fn default_pin_zoom_step() -> f32 {
    0.1
}

fn default_ocr_provider() -> String {
    "local-mnn".to_owned()
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureRegionCommand {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    depth: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayPingCommand {
    kind: String,
    protocol: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayPinSelectionCommand {
    kind: String,
    protocol: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedSnapshotCommand {
    mapping_name: String,
    byte_len: usize,
    width: u32,
    height: u32,
    origin_x: f32,
    origin_y: f32,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayControlResponse {
    kind: &'static str,
    message: Option<String>,
}

struct LoadedSharedSnapshot {
    image: DynamicImage,
    tiles: Vec<SnapshotTile>,
}

#[derive(Debug, Clone, Copy)]
struct CaptureRegion {
    rect: EguiRect,
    depth: u8,
}

fn start_control_server(port: u16, queue: OverlayCommandQueue, ctx: Context) {
    thread::spawn(move || {
        let listener = match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => listener,
            Err(error) => {
                log::error!("failed to bind overlay control port {port}: {error}");
                push_overlay_command(
                    &queue,
                    QueuedOverlayCommand::new(OverlayCommand::Error(format!(
                        "failed to bind overlay control port {port}: {error}"
                    ))),
                );
                ctx.request_repaint();
                return;
            }
        };
        log::info!("overlay control server listening on 127.0.0.1:{port}");

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let mut line = String::new();
                    let result = match stream.try_clone() {
                        Ok(reader) => BufReader::new(reader).read_line(&mut line),
                        Err(error) => Err(error),
                    };
                    match result {
                        Ok(0) => {}
                        Ok(_) => {
                            if is_supported_ping(&line) {
                                log::info!("overlay control ping received");
                                let _ = stream.write_all(
                                    format!(
                                        "{{\"kind\":\"pong\",\"protocol\":{CONTROL_PROTOCOL_VERSION}}}\n"
                                    )
                                    .as_bytes(),
                                );
                            } else if is_supported_pin_selection_command(&line) {
                                log::info!(
                                    "overlay pin-selection command accepted by control thread"
                                );
                                let (completion_tx, completion_rx) = mpsc::channel();
                                push_overlay_command(
                                    &queue,
                                    QueuedOverlayCommand::with_completion(
                                        OverlayCommand::PinSelection,
                                        completion_tx,
                                    ),
                                );
                                ctx.request_repaint();

                                let result = completion_rx
                                    .recv_timeout(Duration::from_millis(COMMAND_ACK_TIMEOUT_MS))
                                    .unwrap_or_else(|_| {
                                        log::error!(
                                            "overlay UI did not ACK pin-selection command within {} ms",
                                            COMMAND_ACK_TIMEOUT_MS
                                        );
                                        Err("resident screenshot overlay did not process the pin-selection command in time".to_owned())
                                    });
                                log::info!(
                                    "overlay pin-selection command ACK result={}",
                                    if result.is_ok() { "ok" } else { "error" }
                                );
                                write_control_response(&mut stream, result);
                            } else {
                                match serde_json::from_str::<OverlayCaptureCommand>(&line) {
                                    Ok(command) => {
                                        log::info!(
                                            "overlay capture command accepted by control thread"
                                        );
                                        let (completion_tx, completion_rx) = mpsc::channel();
                                        push_overlay_command(
                                            &queue,
                                            QueuedOverlayCommand::with_completion(
                                                OverlayCommand::Capture(command),
                                                completion_tx,
                                            ),
                                        );
                                        ctx.request_repaint();

                                        let result = completion_rx
                                            .recv_timeout(Duration::from_millis(
                                                COMMAND_ACK_TIMEOUT_MS,
                                            ))
                                            .unwrap_or_else(|_| {
                                                log::error!(
                                                    "overlay UI did not ACK capture command within {} ms",
                                                    COMMAND_ACK_TIMEOUT_MS
                                                );
                                                Err("resident screenshot overlay did not process the capture command in time".to_owned())
                                            });
                                        log::info!(
                                            "overlay capture command ACK result={}",
                                            if result.is_ok() { "ok" } else { "error" }
                                        );
                                        write_control_response(&mut stream, result);
                                    }
                                    Err(error) => {
                                        let message =
                                            format!("failed to parse overlay command: {error}");
                                        push_overlay_command(
                                            &queue,
                                            QueuedOverlayCommand::new(OverlayCommand::Error(
                                                message.clone(),
                                            )),
                                        );
                                        ctx.request_repaint();
                                        write_control_response(&mut stream, Err(message));
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            log::error!("failed to read overlay command: {error}");
                            push_overlay_command(
                                &queue,
                                QueuedOverlayCommand::new(OverlayCommand::Error(format!(
                                    "failed to read overlay command: {error}"
                                ))),
                            )
                        }
                    }
                }
                Err(error) => {
                    log::error!("overlay control connection failed: {error}");
                    push_overlay_command(
                        &queue,
                        QueuedOverlayCommand::new(OverlayCommand::Error(format!(
                            "overlay control connection failed: {error}"
                        ))),
                    );
                    ctx.request_repaint();
                }
            }
        }
    });
}

impl QueuedOverlayCommand {
    fn new(command: OverlayCommand) -> Self {
        Self {
            command,
            completion: None,
        }
    }

    fn with_completion(command: OverlayCommand, completion: Sender<Result<(), String>>) -> Self {
        Self {
            command,
            completion: Some(completion),
        }
    }
}

fn push_overlay_command(queue: &OverlayCommandQueue, command: QueuedOverlayCommand) {
    if let Ok(mut queue) = queue.lock() {
        queue.push_back(command);
        log::info!("overlay command queued pending={}", queue.len());
    } else {
        log::error!("overlay command queue lock poisoned");
    }
}

fn write_control_response(stream: &mut impl Write, result: Result<(), String>) {
    let response = match result {
        Ok(()) => OverlayControlResponse {
            kind: "accepted",
            message: None,
        },
        Err(message) => OverlayControlResponse {
            kind: "error",
            message: Some(message),
        },
    };

    let _ = serde_json::to_writer(&mut *stream, &response);
    let _ = stream.write_all(b"\n");
}

fn is_supported_ping(line: &str) -> bool {
    serde_json::from_str::<OverlayPingCommand>(line)
        .is_ok_and(|command| command.kind == "ping" && command.protocol == CONTROL_PROTOCOL_VERSION)
}

fn is_supported_pin_selection_command(line: &str) -> bool {
    serde_json::from_str::<OverlayPinSelectionCommand>(line).is_ok_and(|command| {
        command.kind == "pinSelection" && command.protocol == CONTROL_PROTOCOL_VERSION
    })
}

fn park_resident_window(ctx: &Context, hwnd: Option<isize>) {
    // Keep the resident window alive but invisible to the user. Fully hidden
    // windows may stop receiving repaint wakeups, leaving future commands queued.
    if let Some(hwnd) = hwnd {
        platform_win32::park_window(
            hwnd,
            Rect::new(
                Point::new(RESIDENT_IDLE_X, RESIDENT_IDLE_Y),
                Size::new(RESIDENT_IDLE_SIZE, RESIDENT_IDLE_SIZE),
            ),
            true,
        );
    }
    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(
        RESIDENT_IDLE_X,
        RESIDENT_IDLE_Y,
    )));
    ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
        RESIDENT_IDLE_SIZE,
        RESIDENT_IDLE_SIZE,
    )));
    ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
}

fn request_resident_idle_repaint(ctx: &Context) {
    ctx.request_repaint_after(Duration::from_millis(RESIDENT_IDLE_REPAINT_MS));
}

fn show_capture_window(ctx: &Context, snapshot: &SharedSnapshotCommand) {
    ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(
        snapshot.origin_x,
        snapshot.origin_y,
    )));
    ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
        snapshot.width as f32,
        snapshot.height as f32,
    )));
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Focus);
    ctx.request_repaint();
}

fn hwnd_from_raw_window_handle(handle: RawWindowHandle) -> Option<isize> {
    match handle {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

fn build_capture_regions(
    regions: &[CaptureRegionCommand],
    snapshot: &SharedSnapshotCommand,
) -> Vec<CaptureRegion> {
    let canvas = EguiRect::from_min_size(
        Pos2::ZERO,
        Vec2::new(snapshot.width as f32, snapshot.height as f32),
    );
    let mut capture_regions = regions
        .iter()
        .filter_map(|region| {
            let rect = EguiRect::from_min_size(
                Pos2::new(region.x - snapshot.origin_x, region.y - snapshot.origin_y),
                Vec2::new(region.width, region.height),
            )
            .intersect(canvas);

            (rect.width() >= MIN_SELECTION_SIZE && rect.height() >= MIN_SELECTION_SIZE).then_some(
                CaptureRegion {
                    rect,
                    depth: region.depth,
                },
            )
        })
        .collect::<Vec<_>>();

    capture_regions.sort_by(|a, b| {
        a.rect
            .area()
            .total_cmp(&b.rect.area())
            .then_with(|| b.depth.cmp(&a.depth))
    });
    capture_regions
}

fn load_shared_snapshot(
    ctx: &Context,
    snapshot: &SharedSnapshotCommand,
    text: OverlayText,
) -> Result<LoadedSharedSnapshot, String> {
    let expected_len = snapshot.width as usize * snapshot.height as usize * 4;
    if snapshot.width == 0 || snapshot.height == 0 || snapshot.byte_len != expected_len {
        return Err(format!(
            "{}: invalid shared snapshot dimensions",
            text.snapshot_load_failed
        ));
    }

    let bytes = platform_win32::read_named_shared_memory(&snapshot.mapping_name, snapshot.byte_len)
        .map_err(|error| format!("{}: {error}", text.snapshot_load_failed))?;
    let rgba = image::RgbaImage::from_raw(snapshot.width, snapshot.height, bytes)
        .ok_or_else(|| format!("{}: invalid RGBA buffer", text.snapshot_load_failed))?;
    let tiles = build_snapshot_tiles(ctx, &rgba);

    Ok(LoadedSharedSnapshot {
        image: DynamicImage::ImageRgba8(rgba),
        tiles,
    })
}

fn load_snapshot(
    ctx: &Context,
    path: Option<&PathBuf>,
    text: OverlayText,
) -> (Option<DynamicImage>, Vec<SnapshotTile>, Option<String>) {
    let Some(path) = path else {
        return (None, Vec::new(), Some(text.missing_snapshot.to_owned()));
    };

    match image::open(path) {
        Ok(image) => {
            let rgba = image.to_rgba8();
            let tiles = build_snapshot_tiles(ctx, &rgba);
            (Some(DynamicImage::ImageRgba8(rgba)), tiles, None)
        }
        Err(error) => (
            None,
            Vec::new(),
            Some(format!("{}: {error}", text.snapshot_load_failed)),
        ),
    }
}

struct SnapshotTile {
    texture: TextureHandle,
    rect: EguiRect,
}

fn build_snapshot_tiles(ctx: &Context, image: &image::RgbaImage) -> Vec<SnapshotTile> {
    let max_texture_side = ctx.input(|input| input.max_texture_side.max(1)) as u32;
    let tile_side = max_texture_side.min(1024).max(1);
    let image_width = image.width();
    let image_height = image.height();
    let mut tiles = Vec::new();

    let mut y = 0;
    while y < image_height {
        let height = tile_side.min(image_height - y);
        let mut x = 0;
        while x < image_width {
            let width = tile_side.min(image_width - x);
            let tile = image::imageops::crop_imm(image, x, y, width, height).to_image();
            let size = [width as usize, height as usize];
            let texture = ctx.load_texture(
                format!("screen-snapshot-{x}-{y}"),
                ColorImage::from_rgba_unmultiplied(size, tile.as_raw()),
                TextureOptions::LINEAR,
            );
            let rect = EguiRect::from_min_size(
                Pos2::new(x as f32, y as f32),
                Vec2::new(width as f32, height as f32),
            );
            tiles.push(SnapshotTile { texture, rect });

            x += width;
        }

        y += height;
    }

    tiles
}

struct CroppedSnapshot {
    path: PathBuf,
    width: u32,
    height: u32,
}

fn crop_snapshot_to_file(
    snapshot: &DynamicImage,
    selection: EguiRect,
    text: &OverlayText,
) -> Result<CroppedSnapshot, String> {
    let cropped = crop_snapshot(snapshot, selection);
    let width = cropped.width();
    let height = cropped.height();
    let image_path = std::env::temp_dir().join(format!(
        "snap_pin_capture_{}.png",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    ));

    cropped
        .save(&image_path)
        .map_err(|error| format!("{}: {error}", text.crop_failed))?;
    Ok(CroppedSnapshot {
        path: image_path,
        width,
        height,
    })
}

fn save_snapshot_to_file(
    snapshot: &DynamicImage,
    selection: EguiRect,
    text: &OverlayText,
) -> Result<PathBuf, String> {
    let cropped = crop_snapshot(snapshot, selection);
    let default_name = capture_file_name();
    log::info!("prompting save path default_name={default_name}");
    let Some(image_path) = platform_win32::prompt_save_png_path(&default_name)
        .map_err(|error| format!("{}: {error}", text.save_failed))?
    else {
        log::info!("save path prompt canceled");
        return Err(SAVE_CANCELED_CODE.to_owned());
    };

    log::info!("saving cropped snapshot to {}", image_path.display());
    if let Some(parent) = image_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", text.save_failed))?;
    }

    cropped
        .save(&image_path)
        .map_err(|error| format!("{}: {error}", text.save_failed))?;
    log::info!("saved cropped snapshot to {}", image_path.display());
    Ok(image_path)
}

fn copy_snapshot_to_clipboard(
    snapshot: &DynamicImage,
    selection: EguiRect,
    text: &OverlayText,
) -> Result<(), String> {
    let cropped = crop_snapshot(snapshot, selection);
    let image = arboard::ImageData {
        width: cropped.width() as usize,
        height: cropped.height() as usize,
        bytes: Cow::Owned(cropped.into_raw()),
    };
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("{}: {error}", text.copy_failed))?;
    clipboard
        .set_image(image)
        .map_err(|error| format!("{}: {error}", text.copy_failed))
}

fn crop_snapshot(snapshot: &DynamicImage, selection: EguiRect) -> image::RgbaImage {
    let (snapshot_width, snapshot_height) = snapshot.dimensions();
    let x = selection.min.x.round().clamp(0.0, snapshot_width as f32) as u32;
    let y = selection.min.y.round().clamp(0.0, snapshot_height as f32) as u32;
    let max_x = selection.max.x.round().clamp(0.0, snapshot_width as f32) as u32;
    let max_y = selection.max.y.round().clamp(0.0, snapshot_height as f32) as u32;
    let width = max_x.saturating_sub(x).max(1);
    let height = max_y.saturating_sub(y).max(1);
    snapshot.crop_imm(x, y, width, height).to_rgba8()
}

fn capture_file_name() -> String {
    format!(
        "snap_pin_capture_{}.png",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    )
}

fn pin_image_id_from_path(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("image")
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || value == '-' || value == '_' {
                value
            } else {
                '_'
            }
        })
        .collect()
}

struct PinWindowLaunch<'a> {
    image_path: &'a PathBuf,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    opacity: f32,
    zoom_step: f32,
    always_on_top: bool,
    ocr_provider: &'a str,
    ocr_language_hint: Option<&'a str>,
    ocr_default_model_id: Option<&'a str>,
    ocr_models_registry: Option<&'a std::path::Path>,
}

fn spawn_pin_window(launch: &PinWindowLaunch<'_>) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    log::info!(
        "spawning pin window exe={} image={} x={} y={} width={} height={} opacity={} zoom_step={} always_on_top={}",
        current_exe.display(),
        launch.image_path.display(),
        launch.x,
        launch.y,
        launch.width,
        launch.height,
        launch.opacity,
        launch.zoom_step,
        launch.always_on_top
    );
    let child = std::process::Command::new(current_exe)
        .arg("--pin")
        .arg("--image")
        .arg(launch.image_path)
        .arg("--x")
        .arg(format!("{}", launch.x))
        .arg("--y")
        .arg(format!("{}", launch.y))
        .arg("--width")
        .arg(format!("{}", launch.width))
        .arg("--height")
        .arg(format!("{}", launch.height))
        .arg("--pin-opacity")
        .arg(format!("{}", launch.opacity))
        .arg("--pin-zoom-step")
        .arg(format!("{}", launch.zoom_step))
        .arg("--pin-always-on-top")
        .arg(format!("{}", launch.always_on_top))
        .arg("--ocr-provider")
        .arg(launch.ocr_provider)
        .arg("--ocr-language-hint")
        .arg(launch.ocr_language_hint.unwrap_or(""))
        .arg("--ocr-default-model-id")
        .arg(launch.ocr_default_model_id.unwrap_or(""))
        .arg("--ocr-models-registry")
        .arg(
            launch
                .ocr_models_registry
                .unwrap_or_else(|| std::path::Path::new("")),
        )
        .spawn()
        .map_err(|error| error.to_string())?;
    log::info!("pin window spawned pid={}", child.id());
    Ok(())
}

struct PinWindowApp {
    text: OverlayText,
    image_path: Option<PathBuf>,
    texture: Option<TextureHandle>,
    image_size: Vec2,
    image_display_size: Vec2,
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
}

impl PinWindowApp {
    fn new(creation_context: &CreationContext<'_>, args: CliArgs) -> Self {
        install_system_fonts(&creation_context.egui_ctx);
        let text = OverlayText::new(args.language);
        let mut app = Self {
            text,
            image_path: args.image,
            texture: None,
            image_size: Vec2::new(args.width, args.height),
            image_display_size: Vec2::new(args.width, args.height),
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
                self.image_display_size = clamp_pin_window_size(self.image_size);
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
        let new_size = clamp_pin_window_size(current_size * zoom_factor);
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

        if !matches!(action, PinToolbarAction::Close) {
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
        let new_size = pin_window_size_for_image(self.image_display_size, self.toolbar_edge);
        let new_canvas = EguiRect::from_min_size(Pos2::ZERO, new_size);
        let new_image_rect = pin_image_rect(new_canvas, self.image_display_size, self.toolbar_edge);
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

    fn image_rect(&self, canvas: EguiRect) -> EguiRect {
        pin_image_rect(canvas, self.image_display_size, self.toolbar_edge)
    }

    fn toolbar_rect(&self, canvas: EguiRect, image_rect: EguiRect) -> Option<EguiRect> {
        self.toolbar_edge
            .map(|edge| pin_toolbar_rect(canvas, image_rect, edge, self.toolbar_state()))
    }

    fn toolbar_contains(&self, position: Pos2, canvas: EguiRect, image_rect: EguiRect) -> bool {
        self.toolbar_rect(canvas, image_rect)
            .is_some_and(|toolbar| toolbar.expand(4.0).contains(position))
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

    fn drain_ocr_result(&mut self) {
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
                    && ocr_block_interaction_rect(image_rect, image_size, block.bounds)
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

struct PinOcrRequest {
    path: PathBuf,
    provider: OcrProvider,
    language_hint: Option<String>,
    default_model_id: Option<String>,
    models_registry: Option<PathBuf>,
    load_error_prefix: String,
}

fn recognize_pin_image(request: PinOcrRequest) -> Result<OcrResult, String> {
    let image = image::open(&request.path)
        .map_err(|error| format!("{}: {error}", request.load_error_prefix))?
        .to_rgba8();
    let width = image.width();
    let height = image.height();
    let image_id = ImageId::new(format!("pin-{}", pin_image_id_from_path(&request.path)));
    let image_data = ImageData {
        id: image_id.clone(),
        metadata: ImageMetadata {
            id: image_id.clone(),
            pixel_size: Size::new(width as f32, height as f32),
            format: ImageFormat::Rgba8,
            monitor_name: None,
        },
        bytes: image.into_raw(),
    };
    let mut job = OcrJob {
        id: format!("pin-ocr-{}", pin_image_id_from_path(&request.path)),
        image_id,
        source_rect: Some(Rect::new(
            Point::ZERO,
            Size::new(width as f32, height as f32),
        )),
        language_hint: request.language_hint,
        provider: request.provider,
        provider_profile_id: None,
        model_id: request.default_model_id,
    };

    let mut engine = RoutedOcrEngine::default();
    if matches!(job.provider, OcrProvider::Local(_)) {
        let registry = load_model_registry(request.models_registry.as_deref());
        let model = job
            .model_id
            .as_deref()
            .and_then(|model_id| registry.find(model_id))
            .or_else(|| registry.recommended_ocr());

        if let Some(model) = model {
            if job.model_id.is_none() {
                job.model_id = Some(model.id.clone());
            }
            engine.load_model(model).map_err(|error| error.message)?;
        }
    }

    engine
        .recognize(&job, &image_data)
        .map_err(|error| error.message)
}

fn load_model_registry(path: Option<&std::path::Path>) -> ModelRegistry {
    let mut registry = ModelRegistry::with_builtin_defaults();
    let Some(path) = path else {
        return registry;
    };

    match std::fs::read_to_string(path) {
        Ok(contents) => {
            match serde_json::from_str::<Vec<shared_models::ModelManifest>>(&contents) {
                Ok(models) => {
                    for model in models {
                        registry.register(model);
                    }
                }
                Err(error) => {
                    log::error!(
                        "failed to parse OCR model registry {}: {error}",
                        path.display()
                    );
                }
            }
        }
        Err(error) => {
            log::warn!(
                "OCR model registry not loaded from {}: {error}",
                path.display()
            );
        }
    }

    registry
}

fn parse_ocr_provider(value: &str) -> OcrProvider {
    match value {
        "disabled" => OcrProvider::Disabled,
        "system" => OcrProvider::System,
        "local-onnx" => OcrProvider::Local(OcrLocalBackend::OnnxRuntime),
        "local-paddle" => OcrProvider::Local(OcrLocalBackend::PaddleRuntime),
        "api-openai" => OcrProvider::ExternalApi(OcrExternalProvider::OpenAi),
        "api-azure" => OcrProvider::ExternalApi(OcrExternalProvider::AzureVision),
        "api-google" => OcrProvider::ExternalApi(OcrExternalProvider::GoogleVision),
        "api-baidu" => OcrProvider::ExternalApi(OcrExternalProvider::BaiduOcr),
        "api-tencent" => OcrProvider::ExternalApi(OcrExternalProvider::TencentOcr),
        "api-custom" => OcrProvider::ExternalApi(OcrExternalProvider::Custom("custom".to_owned())),
        _ => OcrProvider::Local(OcrLocalBackend::Mnn),
    }
}

fn clamp_pin_window_size(size: Vec2) -> Vec2 {
    let mut size = size;
    if size.x < PIN_MIN_WIDTH {
        size *= PIN_MIN_WIDTH / size.x.max(1.0);
    }
    if size.y < PIN_MIN_HEIGHT {
        size *= PIN_MIN_HEIGHT / size.y.max(1.0);
    }
    if size.x > PIN_MAX_SIDE {
        size *= PIN_MAX_SIDE / size.x;
    }
    if size.y > PIN_MAX_SIDE {
        size *= PIN_MAX_SIDE / size.y;
    }
    size
}

fn fit_pin_image_size_to_canvas(image_size: Vec2, canvas_size: Vec2) -> Vec2 {
    if image_size.x <= 0.0 || image_size.y <= 0.0 || canvas_size.x <= 0.0 || canvas_size.y <= 0.0 {
        return image_size.max(Vec2::splat(1.0));
    }

    let scale = (canvas_size.x / image_size.x).min(canvas_size.y / image_size.y);
    clamp_pin_window_size(image_size * scale)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinToolbarAction {
    CopyImage,
    CopySelectedText,
    CopyAllText,
    SaveImage,
    RunOcr,
    Translate,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinToolbarEdge {
    Top,
    Right,
    Bottom,
    Left,
}

impl PinToolbarEdge {
    fn nearest(canvas: EguiRect, position: Pos2) -> Self {
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
struct PinToolbarState {
    text: OverlayText,
    has_ocr_text: bool,
    has_selected_ocr_text: bool,
}

fn current_viewport_rect(ctx: &Context) -> Option<EguiRect> {
    ctx.input(|input| input.viewport().inner_rect.or(input.viewport().outer_rect))
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

fn pin_toolbar_extent(edge: PinToolbarEdge, button_count: usize) -> f32 {
    let size = pin_toolbar_size(edge, button_count);
    if edge.is_horizontal() {
        size.y + PIN_TOOLBAR_OUTSIDE_GAP + PIN_TOOLBAR_MARGIN
    } else {
        size.x + PIN_TOOLBAR_OUTSIDE_GAP + PIN_TOOLBAR_MARGIN
    }
}

fn pin_window_size_for_image(image_size: Vec2, edge: Option<PinToolbarEdge>) -> Vec2 {
    let mut size = image_size;
    if let Some(edge) = edge {
        let extent = pin_toolbar_extent(edge, PIN_TOOLBAR_MAX_BUTTONS);
        if edge.is_horizontal() {
            size.y += extent;
        } else {
            size.x += extent;
        }
    }

    size
}

fn pin_image_rect(canvas: EguiRect, image_size: Vec2, edge: Option<PinToolbarEdge>) -> EguiRect {
    let min = match edge {
        Some(PinToolbarEdge::Top) => Pos2::new(
            canvas.min.x,
            canvas.min.y + pin_toolbar_extent(PinToolbarEdge::Top, PIN_TOOLBAR_MAX_BUTTONS),
        ),
        Some(PinToolbarEdge::Left) => Pos2::new(
            canvas.min.x + pin_toolbar_extent(PinToolbarEdge::Left, PIN_TOOLBAR_MAX_BUTTONS),
            canvas.min.y,
        ),
        _ => canvas.min,
    };

    EguiRect::from_min_size(min, image_size)
}

fn pin_toolbar_rect(
    canvas: EguiRect,
    image_rect: EguiRect,
    edge: PinToolbarEdge,
    state: PinToolbarState,
) -> EguiRect {
    let size = pin_toolbar_size(edge, pin_toolbar_button_count(state));
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

fn pin_toolbar_button_count(state: PinToolbarState) -> usize {
    let mut count = 5;
    if state.has_ocr_text {
        count += 2;
    }
    count
}

fn pin_toolbar_buttons(toolbar: EguiRect, state: PinToolbarState) -> Vec<PinToolbarButton> {
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
    let mut buttons = vec![
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
            label: if state.has_ocr_text {
                text.rerun_ocr_action
            } else {
                text.ocr_action
            },
            shortcut: None,
            action: PinToolbarAction::RunOcr,
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
    ];

    if state.has_ocr_text {
        buttons.insert(
            2,
            PinToolbarButton {
                rect: EguiRect::from_min_size(
                    button_min(2.0),
                    Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE),
                ),
                label: text.copy_selected_text_action,
                shortcut: Some("Ctrl+C"),
                action: PinToolbarAction::CopySelectedText,
                enabled: state.has_selected_ocr_text,
            },
        );
        buttons.insert(
            3,
            PinToolbarButton {
                rect: EguiRect::from_min_size(
                    button_min(3.0),
                    Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE),
                ),
                label: text.copy_all_text_action,
                shortcut: Some("Ctrl+Shift+C"),
                action: PinToolbarAction::CopyAllText,
                enabled: true,
            },
        );
        for (index, button) in buttons.iter_mut().enumerate() {
            button.rect = EguiRect::from_min_size(
                button_min(index as f32),
                Vec2::splat(PIN_TOOLBAR_BUTTON_SIZE),
            );
        }
    }

    buttons
}

fn pin_toolbar_action_at(
    position: Pos2,
    toolbar: EguiRect,
    state: PinToolbarState,
) -> Option<PinToolbarAction> {
    pin_toolbar_buttons(toolbar, state)
        .into_iter()
        .find(|button| button.enabled && button.rect.contains(position))
        .map(|button| button.action)
}

fn draw_pin_toolbar(
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

    for button in pin_toolbar_buttons(toolbar, state) {
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

fn draw_text_overlay(
    painter: &Painter,
    image_rect: EguiRect,
    image_size: Size,
    overlay: &TextOverlay,
    selected: bool,
) {
    let text = text_overlay_display_text(&overlay.text);
    if text.is_empty() {
        return;
    }

    let bounds_rect = text_overlay_bounds_rect(image_rect, image_size, overlay);
    let font_size = text_overlay_font_size(bounds_rect);
    let label_rect = text_overlay_label_rect(painter, image_rect, bounds_rect, &text, font_size);
    let padding = Vec2::new(OCR_TEXT_OVERLAY_PADDING_X, OCR_TEXT_OVERLAY_PADDING_Y);
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

fn ocr_block_interaction_rect(image_rect: EguiRect, image_size: Size, bounds: Rect) -> EguiRect {
    image_bounds_to_screen(image_rect, image_size, bounds)
        .expand2(Vec2::new(
            OCR_TEXT_INTERACTION_PADDING_X,
            OCR_TEXT_INTERACTION_PADDING_Y,
        ))
        .intersect(image_rect)
}

fn text_overlay_label_rect(
    painter: &Painter,
    image_rect: EguiRect,
    bounds_rect: EguiRect,
    text: &str,
    font_size: f32,
) -> EguiRect {
    let galley = painter.layout(
        text.to_owned(),
        FontId::proportional(font_size),
        Color32::WHITE,
        bounds_rect.width().max(80.0),
    );
    let padding = Vec2::new(OCR_TEXT_OVERLAY_PADDING_X, OCR_TEXT_OVERLAY_PADDING_Y);
    let label_size = Vec2::new(
        (galley.size().x + padding.x * 2.0).min(image_rect.width().max(1.0)),
        galley.size().y + padding.y * 2.0,
    );
    let mut label_min = bounds_rect.left_top();
    label_min.x = label_min.x.clamp(
        image_rect.min.x,
        (image_rect.max.x - label_size.x).max(image_rect.min.x),
    );
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

fn text_overlay_font_size(bounds_rect: EguiRect) -> f32 {
    (bounds_rect.height() * OCR_TEXT_FONT_HEIGHT_RATIO)
        .clamp(OCR_TEXT_FONT_MIN_SIZE, OCR_TEXT_FONT_MAX_SIZE)
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

fn draw_pin_status(painter: &Painter, canvas: EguiRect, status: &str) {
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

impl App for PinWindowApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.drain_ocr_result();
        self.handle_shortcuts(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let canvas = ui.max_rect();
                let response = ui.interact(canvas, Id::new("pin-drag"), Sense::click_and_drag());
                if self.toolbar_edge.is_none() {
                    let fitted_size = fit_pin_image_size_to_canvas(self.image_size, canvas.size());
                    if (self.image_display_size - fitted_size).length_sq() > f32::EPSILON {
                        self.image_display_size = fitted_size;
                    }
                }
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

fn clamp_pos(pos: Pos2, rect: EguiRect) -> Pos2 {
    Pos2::new(
        pos.x.clamp(rect.min.x, rect.max.x),
        pos.y.clamp(rect.min.y, rect.max.y),
    )
}

fn selection_drag_mode(selection: EguiRect, position: Pos2) -> CaptureDragMode {
    let near_left = (position.x - selection.min.x).abs() <= SELECTION_EDGE_HIT_SIZE;
    let near_right = (position.x - selection.max.x).abs() <= SELECTION_EDGE_HIT_SIZE;
    let near_top = (position.y - selection.min.y).abs() <= SELECTION_EDGE_HIT_SIZE;
    let near_bottom = (position.y - selection.max.y).abs() <= SELECTION_EDGE_HIT_SIZE;
    let edges = ResizeEdges {
        left: near_left,
        right: near_right,
        top: near_top,
        bottom: near_bottom,
    };

    if edges.left || edges.right || edges.top || edges.bottom {
        CaptureDragMode::Resize(edges)
    } else if selection.contains(position) {
        CaptureDragMode::Move
    } else {
        CaptureDragMode::Create
    }
}

fn apply_drag(drag: CaptureDragState, position: Pos2, canvas: EguiRect) -> EguiRect {
    match drag.mode {
        CaptureDragMode::Create => normalize_selection(EguiRect::from_two_pos(
            drag.start,
            clamp_pos(position, canvas),
        )),
        CaptureDragMode::Move => {
            let delta = position - drag.start;
            let mut rect = drag.original.translate(delta);
            if rect.min.x < canvas.min.x {
                rect = rect.translate(Vec2::new(canvas.min.x - rect.min.x, 0.0));
            }
            if rect.min.y < canvas.min.y {
                rect = rect.translate(Vec2::new(0.0, canvas.min.y - rect.min.y));
            }
            if rect.max.x > canvas.max.x {
                rect = rect.translate(Vec2::new(canvas.max.x - rect.max.x, 0.0));
            }
            if rect.max.y > canvas.max.y {
                rect = rect.translate(Vec2::new(0.0, canvas.max.y - rect.max.y));
            }
            rect
        }
        CaptureDragMode::Resize(edges) => resize_selection(drag.original, position, canvas, edges),
    }
}

fn resize_selection(
    selection: EguiRect,
    position: Pos2,
    canvas: EguiRect,
    edges: ResizeEdges,
) -> EguiRect {
    let position = clamp_pos(position, canvas);
    let mut min = selection.min;
    let mut max = selection.max;

    if edges.left {
        min.x = position.x.min(max.x - SELECTION_MIN_SIZE);
    }
    if edges.right {
        max.x = position.x.max(min.x + SELECTION_MIN_SIZE);
    }
    if edges.top {
        min.y = position.y.min(max.y - SELECTION_MIN_SIZE);
    }
    if edges.bottom {
        max.y = position.y.max(min.y + SELECTION_MIN_SIZE);
    }

    normalize_selection(EguiRect::from_min_max(
        clamp_pos(min, canvas),
        clamp_pos(max, canvas),
    ))
}

fn normalize_selection(selection: EguiRect) -> EguiRect {
    EguiRect::from_min_max(
        Pos2::new(
            selection.min.x.min(selection.max.x),
            selection.min.y.min(selection.max.y),
        ),
        Pos2::new(
            selection.min.x.max(selection.max.x),
            selection.min.y.max(selection.max.y),
        ),
    )
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
