#![allow(dead_code)]

mod annotation;
mod capture_paint;
mod cli;
mod fonts;
mod hotkeys;
mod input;
mod logging;
mod overlay_state;
mod overlay_text;
mod pin;
mod renderer;
mod text_layer;

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
    self, Align2, Color32, ColorImage, Context, FontId, Id, Key, Painter, Pos2, Rect as EguiRect,
    Sense, TextureHandle, TextureOptions, Vec2, ViewportBuilder, ViewportCommand, WindowLevel,
};
use eframe::{App, CreationContext, Frame, NativeOptions};
use image::{DynamicImage, GenericImageView};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde::Deserialize;
use shared_models::{Point, Rect, Size};

use capture_paint::{
    draw_error, draw_hint, draw_magnifier, draw_pin_border, draw_selection_mask, draw_size_label,
    draw_toolbar, format_color_value, snapshot_color_at, toolbar_action_at,
};
use cli::{CliArgs, OverlayLanguage, OverlayRunMode, parse_color};
use fonts::install_system_fonts;
use hotkeys::{command_shortcut_pressed, copy_shortcut_pressed, hotkey_pressed};
use logging::init_logging;
use overlay_state::OverlayApp;
use overlay_text::OverlayText;

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
        OverlayRunMode::Pin => ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(true)
            .with_taskbar(false)
            .with_position([args.x, args.y])
            .with_inner_size([args.width, args.height])
            .with_min_inner_size([96.0, 72.0]),
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
    show_magnifier: bool,
    magnifier_scale: f32,
    pin_hotkey: String,
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
            show_magnifier: args.show_magnifier,
            magnifier_scale: args.magnifier_scale,
            pin_hotkey: args.pin_hotkey,
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

        if ctx.input(|input| input.key_pressed(Key::Enter) || input.key_pressed(Key::P)) {
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
                    if let Some(action) = toolbar_action_at(clamped, canvas, selection, self.text) {
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
            draw_size_label(painter, selection);
            draw_toolbar(painter, canvas, selection, self.border_color, self.text);
        } else if let Some(region) = self.hovered_region {
            draw_selection_mask(painter, canvas, region, mask_alpha, self.border_color);
            draw_size_label(painter, region);
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
            if let Some(pixel) = hovered_pixel {
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
        self.run_capture_action(ctx, CaptureAction::Pin);
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
        spawn_pin_window(
            &cropped.path,
            region.origin.x,
            region.origin.y,
            cropped.width as f32,
            cropped.height as f32,
        )
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
        self.show_magnifier = command.show_magnifier;
        self.magnifier_scale = command.magnifier_scale.clamp(1.0, 6.0);
        self.pin_hotkey = command.pin_hotkey;
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
                    self.run_capture_action(ctx, CaptureAction::Pin);
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
    #[serde(default = "default_show_magnifier")]
    show_magnifier: bool,
    #[serde(default = "default_magnifier_scale")]
    magnifier_scale: f32,
    #[serde(default = "default_pin_hotkey")]
    pin_hotkey: String,
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

fn spawn_pin_window(
    image_path: &PathBuf,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    log::info!(
        "spawning pin window exe={} image={} x={} y={} width={} height={}",
        current_exe.display(),
        image_path.display(),
        x,
        y,
        width,
        height
    );
    let child = std::process::Command::new(current_exe)
        .arg("--pin")
        .arg("--image")
        .arg(image_path)
        .arg("--x")
        .arg(format!("{}", x))
        .arg("--y")
        .arg(format!("{}", y))
        .arg("--width")
        .arg(format!("{}", width))
        .arg("--height")
        .arg(format!("{}", height))
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
    error: Option<String>,
    opacity: f32,
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
            error: None,
            opacity: 1.0,
        };
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
                self.texture = Some(ctx.load_texture(
                    "pinned-image",
                    ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                    TextureOptions::LINEAR,
                ));
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

        let scroll = ctx.input(|input| input.raw_scroll_delta.y);
        if scroll.abs() > 0.0 && ctx.input(|input| input.modifiers.ctrl) {
            let delta = if scroll > 0.0 { 0.05 } else { -0.05 };
            self.opacity = (self.opacity + delta).clamp(0.2, 1.0);
        }
    }
}

impl App for PinWindowApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.handle_shortcuts(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let canvas = ui.max_rect();
                let response = ui.interact(canvas, Id::new("pin-drag"), Sense::click_and_drag());
                if response.dragged() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                if response.secondary_clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }

                if let Some(texture) = &self.texture {
                    let tint = Color32::from_white_alpha((self.opacity * 255.0).round() as u8);
                    ui.painter().image(
                        texture.id(),
                        canvas,
                        EguiRect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        tint,
                    );
                    draw_pin_border(ui.painter(), canvas);
                } else if let Some(error) = &self.error {
                    draw_error(ui.painter(), canvas, error);
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
