use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ab_glyph::{FontArc, PxScale};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};
use perf_trace::{PerfSpan, log_elapsed};
use platform_api::{
    CaptureWindowRegion, ClipboardPayload, HotkeyRegistration, HotkeyToken, SharedMemoryHandle,
};
use serde::{Deserialize, Serialize};
use shared_models::{
    CaptureCompletionAction, OcrExternalProvider, OcrLocalBackend, OcrProvider, Settings,
    TranslateExternalProvider, TranslateLocalBackend, TranslateProvider,
};
use tauri::{AppHandle, Manager};

use super::overlay_launch::overlay_launch;
use super::snapshot::{SnapshotCapture, capture_snapshot};
use crate::settings::models;
use crate::shell_state::ShellState;

const OVERLAY_CONTROL_PORT: u16 = 47232;
const OVERLAY_CONTROL_PROTOCOL: u32 = 2;
const OVERLAY_READY_TIMEOUT: Duration = Duration::from_millis(15_000);
const OVERLAY_COMMAND_TIMEOUT: Duration = Duration::from_millis(5_500);
const OVERLAY_CONNECT_TIMEOUT: Duration = Duration::from_millis(120);
const OVERLAY_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(1_500);
const RECENT_MAPPING_LIMIT: usize = 4;
const CLIPBOARD_TEXT_MAX_CHARS: usize = 20_000;
const CLIPBOARD_TEXT_MAX_WIDTH: u32 = 900;
const CLIPBOARD_TEXT_PADDING: u32 = 22;
const CLIPBOARD_TEXT_LINE_GAP: u32 = 8;
const CLIPBOARD_TEXT_FONT_SIZE: f32 = 18.0;
const CLIPBOARD_PIN_DEFAULT_X: f32 = 160.0;
const CLIPBOARD_PIN_DEFAULT_Y: f32 = 140.0;

#[derive(Default)]
pub struct CaptureOverlayRuntime {
    process: Option<Child>,
    pin_processes: Vec<Child>,
    recent_mappings: VecDeque<SharedMemoryHandle>,
}

pub struct PinHotkeyListener(Box<dyn HotkeyToken>);

struct PreparedPinImage {
    path: PathBuf,
    width: u32,
    height: u32,
}

struct PinWindowLaunch<'a> {
    image_path: &'a Path,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    opacity: f32,
    zoom_step: f32,
    min_width: f32,
    min_height: f32,
    always_on_top: bool,
    language: &'a str,
    ocr_provider: &'a str,
    ocr_language_hint: Option<&'a str>,
    ocr_default_model_id: Option<&'a str>,
    ocr_models_registry: Option<&'a str>,
    translate_provider: &'a str,
    translate_target_language: &'a str,
    translate_segmentation_mode: &'a str,
    translate_default_model_id: Option<&'a str>,
    smart_merge_edge_tolerance_lines: f32,
    smart_merge_loose_edge_tolerance_lines: f32,
    smart_merge_height_ratio_limit: f32,
    smart_merge_longer_line_ratio: f32,
    smart_merge_short_last_line_ratio: f32,
    smart_merge_inline_label_max_chars: usize,
    ocr_text_font_height_ratio: f32,
    ocr_text_min_font_size: f32,
    ocr_text_max_font_size: f32,
    ocr_text_padding_x: f32,
    ocr_text_padding_y: f32,
    ocr_text_interaction_padding_x: f32,
    ocr_text_interaction_padding_y: f32,
}

enum OverlayPinHotkeyResult {
    Pinned,
    Inactive,
    ActiveWithoutSelection,
}

impl CaptureOverlayRuntime {
    fn keep_mapping(&mut self, mapping: SharedMemoryHandle) {
        self.recent_mappings.push_back(mapping);
        while self.recent_mappings.len() > RECENT_MAPPING_LIMIT {
            self.recent_mappings.pop_front();
        }
    }

    fn track_pin_process(&mut self, process: Child) {
        self.prune_pin_processes();
        log::info!("tracking pin window process pid={}", process.id());
        self.pin_processes.push(process);
    }

    fn prune_pin_processes(&mut self) {
        self.pin_processes
            .retain_mut(|process| match process.try_wait() {
                Ok(Some(status)) => {
                    log::info!(
                        "tracked pin window process exited pid={} status={status}",
                        process.id()
                    );
                    false
                }
                Ok(None) => true,
                Err(error) => {
                    log::warn!(
                        "failed to inspect tracked pin window process pid={}: {error}",
                        process.id()
                    );
                    false
                }
            });
    }

    fn stop_resident_overlay(&mut self) {
        if self.process.is_none() {
            return;
        }

        log::info!("requesting resident overlay shutdown");
        if let Err(error) = send_shutdown_command() {
            log::warn!("resident overlay shutdown command failed: {error}");
        } else if let Err(error) = wait_for_overlay_server_shutdown(OVERLAY_SHUTDOWN_TIMEOUT) {
            log::warn!("resident overlay did not close after shutdown command: {error}");
        }

        if let Some(mut process) = self.process.take() {
            match process.try_wait() {
                Ok(Some(status)) => {
                    log::info!(
                        "resident overlay process already exited pid={} status={status}",
                        process.id()
                    );
                }
                Ok(None) => {
                    log::info!("stopping resident overlay process pid={}", process.id());
                    let _ = process.kill();
                    let _ = process.wait();
                }
                Err(error) => {
                    log::warn!(
                        "failed to inspect resident overlay process pid={}: {error}",
                        process.id()
                    );
                    let _ = process.kill();
                    let _ = process.wait();
                }
            }
        }
    }

    fn stop_pin_processes(&mut self) {
        self.prune_pin_processes();
        for mut process in self.pin_processes.drain(..) {
            match process.try_wait() {
                Ok(Some(status)) => {
                    log::info!(
                        "tracked pin window process already exited pid={} status={status}",
                        process.id()
                    );
                }
                Ok(None) => {
                    log::info!("stopping tracked pin window process pid={}", process.id());
                    let _ = process.kill();
                    let _ = process.wait();
                }
                Err(error) => {
                    log::warn!(
                        "failed to inspect tracked pin window process pid={}: {error}",
                        process.id()
                    );
                    let _ = process.kill();
                    let _ = process.wait();
                }
            }
        }
    }
}

impl Drop for CaptureOverlayRuntime {
    fn drop(&mut self) {
        self.stop_resident_overlay();
        self.stop_pin_processes();
    }
}

pub fn launch_capture_overlay(app: &AppHandle) -> Result<(), String> {
    log::info!("launch_capture_overlay requested");
    let settings = current_settings(app)?;
    launch_capture_overlay_for_settings(app, &settings)
}

pub fn launch_capture_overlay_for_settings(
    app: &AppHandle,
    settings: &Settings,
) -> Result<(), String> {
    let mut span = PerfSpan::new("capture_overlay_launch_total")
        .field("include_cursor", settings.capture.include_cursor)
        .field("delay_ms", settings.capture.capture_delay_ms);
    log::info!(
        "launching capture overlay include_cursor={} language={}",
        settings.capture.include_cursor,
        settings.interface.language
    );
    let resident_start = std::time::Instant::now();
    ensure_overlay_resident_for_settings(app, settings)?;
    log_elapsed("capture_overlay_ensure_resident", resident_start);

    if settings.capture.capture_delay_ms > 0 {
        let delay_start = std::time::Instant::now();
        thread::sleep(Duration::from_millis(settings.capture.capture_delay_ms));
        log_elapsed("capture_overlay_delay_wait", delay_start);
    }

    let model_registry_start = std::time::Instant::now();
    let model_registry_path = models::models_path(app)
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    log_elapsed("capture_overlay_model_registry_path", model_registry_start);
    let snapshot_start = std::time::Instant::now();
    let snapshot = capture_snapshot(settings.capture.include_cursor)?;
    log_elapsed("capture_overlay_capture_snapshot", snapshot_start);
    let command = OverlayCaptureCommand::from_settings(settings, &snapshot)
        .with_model_registry_path(model_registry_path.clone());
    let managed = app.state::<Mutex<CaptureOverlayRuntime>>();
    let lock_start = std::time::Instant::now();
    let mut runtime = managed
        .lock()
        .map_err(|_| "overlay runtime lock poisoned".to_owned())?;
    log_elapsed("capture_overlay_runtime_lock", lock_start);

    let keep_mapping_start = std::time::Instant::now();
    runtime.keep_mapping(snapshot.mapping);
    log_elapsed("capture_overlay_keep_mapping", keep_mapping_start);

    let command_start = std::time::Instant::now();
    if let Err(first_error) = send_capture_command(&command) {
        log::warn!("capture command failed; restarting resident overlay: {first_error}");
        let restart_start = std::time::Instant::now();
        ensure_overlay_resident_locked(app, settings, &mut runtime)?;
        log_elapsed("capture_overlay_restart_resident", restart_start);
        send_capture_command(&command)
            .map_err(|second_error| format!("{first_error}; retry failed: {second_error}"))?;
    }
    log_elapsed("capture_overlay_send_capture_command", command_start);

    log::info!(
        "capture overlay launched snapshot={}x{} bytes={}",
        command.snapshot.width,
        command.snapshot.height,
        command.snapshot.byte_len
    );
    span.add_field("width", command.snapshot.width);
    span.add_field("height", command.snapshot.height);
    span.add_field("bytes", command.snapshot.byte_len);
    span.finish();

    Ok(())
}

pub fn ensure_overlay_resident(app: &AppHandle) -> Result<(), String> {
    log::info!("ensuring resident overlay");
    let settings = current_settings(app)?;
    ensure_overlay_resident_for_settings(app, &settings)
}

fn ensure_overlay_resident_for_settings(
    app: &AppHandle,
    settings: &Settings,
) -> Result<(), String> {
    let managed = app.state::<Mutex<CaptureOverlayRuntime>>();
    let mut runtime = managed
        .lock()
        .map_err(|_| "overlay runtime lock poisoned".to_owned())?;
    ensure_overlay_resident_locked(app, settings, &mut runtime)
}

pub fn register_capture_hotkey(app: &AppHandle) -> Result<(), String> {
    let settings = current_settings(app)?;
    register_capture_hotkey_for_settings(app, &settings)
}

pub fn register_global_hotkeys(app: &AppHandle) -> Result<(), String> {
    let settings = current_settings(app)?;
    register_global_hotkeys_for_settings(app, &settings)
}

pub fn register_global_hotkeys_for_settings(
    app: &AppHandle,
    settings: &Settings,
) -> Result<(), String> {
    register_capture_hotkey_for_settings(app, settings)?;
    register_clipboard_pin_hotkey_for_settings(app, settings)
}

pub fn register_capture_hotkey_for_settings(
    app: &AppHandle,
    settings: &Settings,
) -> Result<(), String> {
    log::info!(
        "registering capture hotkey accelerator={}",
        settings.hotkeys.capture
    );
    let registration = HotkeyRegistration::new("capture", settings.hotkeys.capture.clone());
    let app_handle = app.clone();
    let managed = app.state::<Mutex<Option<Box<dyn HotkeyToken>>>>();
    let mut guard = managed
        .lock()
        .map_err(|_| "hotkey listener lock poisoned".to_owned())?;
    *guard = None;

    let listener = platform_runtime::create_platform()
        .global_hotkey()
        .register(
            registration,
            Box::new(move |_| {
                log::info!("capture hotkey triggered");
                if let Err(error) = launch_capture_overlay(&app_handle) {
                    log::error!("failed to launch capture overlay from hotkey: {error}");
                }
            }),
        )
        .map_err(|error| error.to_string())?;

    *guard = Some(listener);
    log::info!("capture hotkey registered");

    Ok(())
}

pub fn register_clipboard_pin_hotkey_for_settings(
    app: &AppHandle,
    settings: &Settings,
) -> Result<(), String> {
    log::info!(
        "registering pin hotkey accelerator={}",
        settings.hotkeys.pin_selection
    );
    let registration = HotkeyRegistration::new("pin", settings.hotkeys.pin_selection.clone());
    let app_handle = app.clone();
    let managed = app.state::<Mutex<Option<PinHotkeyListener>>>();
    let mut guard = managed
        .lock()
        .map_err(|_| "pin hotkey listener lock poisoned".to_owned())?;
    *guard = None;

    let listener = platform_runtime::create_platform()
        .global_hotkey()
        .register(
            registration,
            Box::new(move |_| {
                log::info!("pin hotkey triggered");
                match try_pin_selection_in_overlay() {
                    OverlayPinHotkeyResult::Pinned => {
                        log::info!("pin hotkey consumed by active capture overlay");
                    }
                    OverlayPinHotkeyResult::Inactive => {
                        if let Err(error) = pin_clipboard_content(&app_handle) {
                            log::error!("failed to pin clipboard content from hotkey: {error}");
                        }
                    }
                    OverlayPinHotkeyResult::ActiveWithoutSelection => {
                        log::info!("capture overlay is active but has no selection to pin");
                    }
                }
            }),
        )
        .map_err(|error| error.to_string())?;

    *guard = Some(PinHotkeyListener(listener));
    log::info!("pin hotkey registered");

    Ok(())
}

fn current_settings(app: &AppHandle) -> Result<Settings, String> {
    let state = app.state::<Mutex<ShellState>>();
    state
        .lock()
        .map(|state| state.settings().clone())
        .map_err(|_| "shell state lock poisoned".to_owned())
}

fn ensure_overlay_resident_locked(
    app: &AppHandle,
    settings: &Settings,
    runtime: &mut CaptureOverlayRuntime,
) -> Result<(), String> {
    let span = PerfSpan::new("overlay_resident_ensure_locked");
    if overlay_server_ready() {
        if runtime.process.is_some() {
            log::info!("resident overlay server already ready");
            span.finish();
            return Ok(());
        }

        log::warn!("resident overlay server is ready but untracked; requesting shutdown");
        let shutdown_start = std::time::Instant::now();
        send_shutdown_command()?;
        wait_for_overlay_server_shutdown(OVERLAY_SHUTDOWN_TIMEOUT)?;
        log_elapsed("overlay_resident_shutdown_untracked", shutdown_start);
    }

    if let Some(process) = runtime.process.as_mut() {
        let inspect_start = std::time::Instant::now();
        if process
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            log::warn!("tracked resident overlay process exited");
            runtime.process = None;
        }
        log_elapsed("overlay_resident_inspect_process", inspect_start);
    }

    if let Some(mut process) = runtime.process.take() {
        log::info!("restarting resident overlay process pid={}", process.id());
        let kill_start = std::time::Instant::now();
        let _ = process.kill();
        let _ = process.wait();
        log_elapsed("overlay_resident_kill_previous", kill_start);
    }

    if runtime.process.is_none() {
        let resolve_start = std::time::Instant::now();
        let launch = overlay_launch(app)?;
        log_elapsed("overlay_resident_resolve_launch", resolve_start);
        let registry_start = std::time::Instant::now();
        let model_registry_path = models::models_path(app)
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        log_elapsed("overlay_resident_model_registry_path", registry_start);
        let args_start = std::time::Instant::now();
        let args =
            resident_overlay_args(settings, model_registry_path.as_deref(), std::process::id());
        log_elapsed("overlay_resident_build_args", args_start);
        log::info!("starting resident overlay via {}", launch.description());
        let command_start = std::time::Instant::now();
        let mut command = launch.command(args);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        log_elapsed("overlay_resident_build_command", command_start);
        let spawn_start = std::time::Instant::now();
        let child = command
            .spawn()
            .map_err(|error| format!("failed to launch resident screenshot overlay: {error}"))?;
        log_elapsed("overlay_resident_spawn_process", spawn_start);
        log::info!("resident overlay process started pid={}", child.id());
        runtime.process = Some(child);
    }

    let wait_start = std::time::Instant::now();
    let result = wait_for_overlay_server(runtime.process.as_mut(), OVERLAY_READY_TIMEOUT);
    log_elapsed("overlay_resident_wait_ready", wait_start);
    if result.is_ok() {
        span.finish();
    }
    result
}

fn overlay_server_ready() -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&control_addr(), OVERLAY_CONNECT_TIMEOUT)
    else {
        return false;
    };

    let _ = stream.set_read_timeout(Some(OVERLAY_CONNECT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(OVERLAY_CONNECT_TIMEOUT));
    let ping = format!("{{\"kind\":\"ping\",\"protocol\":{OVERLAY_CONTROL_PROTOCOL}}}\n");
    if stream.write_all(ping.as_bytes()).is_err() {
        return false;
    }

    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .is_ok_and(|count| count > 0 && response.contains("\"kind\":\"pong\""))
}

fn wait_for_overlay_server(
    mut process: Option<&mut Child>,
    timeout: Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if overlay_server_ready() {
            log::info!(
                "resident overlay became ready after {} ms",
                start.elapsed().as_millis()
            );
            return Ok(());
        }

        if let Some(process) = process.as_deref_mut() {
            if let Some(status) = process.try_wait().map_err(|error| error.to_string())? {
                log::error!("resident overlay exited before ready status={status}");
                return Err(format!(
                    "resident screenshot overlay exited before becoming ready: {status}"
                ));
            }
        }

        thread::sleep(Duration::from_millis(30));
    }

    Err(format!(
        "resident screenshot overlay did not become ready within {} ms",
        timeout.as_millis()
    ))
}

fn send_capture_command(command: &OverlayCaptureCommand) -> Result<(), String> {
    let json = serde_json::to_string(command).map_err(|error| error.to_string())?;
    send_overlay_command(&json, "screenshot")
}

fn try_pin_selection_in_overlay() -> OverlayPinHotkeyResult {
    let command = format!("{{\"kind\":\"pinSelection\",\"protocol\":{OVERLAY_CONTROL_PROTOCOL}}}");
    match send_overlay_command(&command, "pin selection") {
        Ok(()) => OverlayPinHotkeyResult::Pinned,
        Err(error) if error == "capture_overlay_inactive" => OverlayPinHotkeyResult::Inactive,
        Err(error) => {
            log::info!("resident overlay did not consume pin hotkey: {error}");
            if error.starts_with("failed to connect to resident screenshot overlay") {
                OverlayPinHotkeyResult::Inactive
            } else {
                OverlayPinHotkeyResult::ActiveWithoutSelection
            }
        }
    }
}

fn send_shutdown_command() -> Result<(), String> {
    let command = format!("{{\"kind\":\"shutdown\",\"protocol\":{OVERLAY_CONTROL_PROTOCOL}}}");
    send_overlay_command(&command, "shutdown")
}

fn send_overlay_command(json: &str, label: &str) -> Result<(), String> {
    let span = PerfSpan::new("overlay_control_command").field("label", label);
    let connect_start = std::time::Instant::now();
    let mut stream = TcpStream::connect_timeout(&control_addr(), OVERLAY_CONNECT_TIMEOUT)
        .map_err(|error| format!("failed to connect to resident screenshot overlay: {error}"))?;
    log_elapsed("overlay_control_connect", connect_start);
    let _ = stream.set_write_timeout(Some(OVERLAY_COMMAND_TIMEOUT));
    let _ = stream.set_read_timeout(Some(OVERLAY_COMMAND_TIMEOUT));
    let write_start = std::time::Instant::now();
    stream
        .write_all(json.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("failed to send {label} command: {error}"))?;
    log_elapsed("overlay_control_write", write_start);

    let mut response = String::new();
    let read_start = std::time::Instant::now();
    let count = BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|error| format!("failed to read {label} command response: {error}"))?;
    log_elapsed("overlay_control_read_response", read_start);
    if count == 0 {
        return Err(format!(
            "resident screenshot overlay closed the {label} command connection"
        ));
    }

    match serde_json::from_str::<OverlayControlResponse>(&response) {
        Ok(response) if response.kind == "accepted" => {
            log::info!("resident overlay accepted {label} command");
            span.finish();
            Ok(())
        }
        Ok(response) if response.kind == "error" => {
            let message = response.message.unwrap_or_else(|| {
                format!("resident screenshot overlay rejected the {label} command")
            });
            if label == "pin selection" && message == "capture_overlay_inactive" {
                log::info!(
                    "resident overlay skipped {label} command because capture overlay is inactive"
                );
            } else {
                log::error!("resident overlay rejected {label} command: {message}");
            }
            Err(message)
        }
        Ok(response) => Err(format!(
            "resident screenshot overlay returned unexpected response '{}'",
            response.kind
        )),
        Err(error) => Err(format!(
            "failed to parse screenshot command response: {error}; response was {response:?}"
        )),
    }
}

fn wait_for_overlay_server_shutdown(timeout: Duration) -> Result<(), String> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if !overlay_server_ready() {
            log::info!(
                "resident overlay closed after {} ms",
                start.elapsed().as_millis()
            );
            return Ok(());
        }

        thread::sleep(Duration::from_millis(30));
    }

    Err(format!(
        "resident screenshot overlay did not close within {} ms",
        timeout.as_millis()
    ))
}

fn pin_clipboard_content(app: &AppHandle) -> Result<(), String> {
    let settings = current_settings(app)?;
    let model_registry_path = models::models_path(app)
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    let payload = platform_runtime::create_platform()
        .clipboard()
        .read()
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let image = clipboard_payload_to_pin_image(app, payload)?;
    let ocr_provider = ocr_provider_name(&settings.ocr.provider);
    let translate_provider = translate_provider_name(&settings.translate.provider);

    spawn_pin_window_from_desktop(
        app,
        &PinWindowLaunch {
            image_path: &image.path,
            x: CLIPBOARD_PIN_DEFAULT_X,
            y: CLIPBOARD_PIN_DEFAULT_Y,
            width: image.width as f32,
            height: image.height as f32,
            opacity: settings.pin.default_opacity,
            zoom_step: settings.pin.zoom_step,
            min_width: settings.pin.min_width,
            min_height: settings.pin.min_height,
            always_on_top: settings.pin.always_on_top,
            language: &settings.interface.language,
            ocr_provider: &ocr_provider,
            ocr_language_hint: settings.ocr.language_hint.as_deref(),
            ocr_default_model_id: settings.ocr.default_model_id.as_deref(),
            ocr_models_registry: model_registry_path.as_deref(),
            translate_provider: &translate_provider,
            translate_target_language: &settings.translate.target_language,
            translate_segmentation_mode: &settings.translate.segmentation_mode,
            translate_default_model_id: settings.translate.default_model_id.as_deref(),
            smart_merge_edge_tolerance_lines: settings.translate.smart_merge.edge_tolerance_lines,
            smart_merge_loose_edge_tolerance_lines: settings
                .translate
                .smart_merge
                .loose_edge_tolerance_lines,
            smart_merge_height_ratio_limit: settings.translate.smart_merge.height_ratio_limit,
            smart_merge_longer_line_ratio: settings.translate.smart_merge.longer_line_ratio,
            smart_merge_short_last_line_ratio: settings.translate.smart_merge.short_last_line_ratio,
            smart_merge_inline_label_max_chars: settings
                .translate
                .smart_merge
                .inline_label_max_chars,
            ocr_text_font_height_ratio: settings.pin.ocr_text.font_height_ratio,
            ocr_text_min_font_size: settings.pin.ocr_text.min_font_size,
            ocr_text_max_font_size: settings.pin.ocr_text.max_font_size,
            ocr_text_padding_x: settings.pin.ocr_text.padding_x,
            ocr_text_padding_y: settings.pin.ocr_text.padding_y,
            ocr_text_interaction_padding_x: settings.pin.ocr_text.interaction_padding_x,
            ocr_text_interaction_padding_y: settings.pin.ocr_text.interaction_padding_y,
        },
    )
}

fn clipboard_payload_to_pin_image(
    app: &AppHandle,
    payload: ClipboardPayload,
) -> Result<PreparedPinImage, String> {
    match payload {
        ClipboardPayload::Files(files) => clipboard_files_to_pin_image(app, files),
        ClipboardPayload::ImageRgba {
            width,
            height,
            bytes,
        } => {
            let rgba = RgbaImage::from_raw(width as u32, height as u32, bytes)
                .ok_or_else(|| "clipboard image has an invalid RGBA buffer".to_owned())?;
            save_dynamic_pin_image(app, DynamicImage::ImageRgba8(rgba), "clipboard_image")
        }
        ClipboardPayload::Text(text) => {
            let image = render_text_to_image(&text)?;
            save_dynamic_pin_image(app, DynamicImage::ImageRgba8(image), "clipboard_text")
        }
    }
}

fn clipboard_files_to_pin_image(
    app: &AppHandle,
    files: Vec<PathBuf>,
) -> Result<PreparedPinImage, String> {
    for file in files.iter().filter(|path| path.is_file()) {
        match image::open(file) {
            Ok(image) => {
                log::info!("pinning copied image file {}", file.display());
                return save_dynamic_pin_image(app, image, "clipboard_file");
            }
            Err(error) => {
                log::info!(
                    "copied file is not a supported image {}: {error}",
                    file.display()
                );
            }
        }
    }

    if files.is_empty() {
        return Err("clipboard file list is empty".to_owned());
    }

    let display = files
        .iter()
        .map(|path| path.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n");
    let image = render_text_to_image(&display)?;
    save_dynamic_pin_image(app, DynamicImage::ImageRgba8(image), "clipboard_files")
}

fn save_dynamic_pin_image(
    app: &AppHandle,
    image: DynamicImage,
    prefix: &str,
) -> Result<PreparedPinImage, String> {
    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let path = clipboard_pin_temp_path(app, prefix)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create pin temp dir: {error}"))?;
    }

    DynamicImage::ImageRgba8(rgba)
        .save(&path)
        .map_err(|error| format!("failed to save clipboard pin image: {error}"))?;
    Ok(PreparedPinImage {
        path,
        width,
        height,
    })
}

fn clipboard_pin_temp_path(app: &AppHandle, prefix: &str) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_cache_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|error| error.to_string())?
        .join("clipboard_pins");
    Ok(root.join(format!("{prefix}_{}.png", timestamp_millis())))
}

fn render_text_to_image(text: &str) -> Result<RgbaImage, String> {
    let font = load_clipboard_pin_font()
        .ok_or_else(|| "failed to load a system font for clipboard text pinning".to_owned())?;
    let scale = PxScale::from(CLIPBOARD_TEXT_FONT_SIZE);
    let text = truncate_clipboard_text(text);
    let lines = wrap_text_lines(&font, scale, &text, CLIPBOARD_TEXT_MAX_WIDTH);
    let lines = if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    };
    let line_height = text_size(scale, &font, "Hg").1.max(22);
    let content_width = lines
        .iter()
        .map(|line| text_size(scale, &font, line).0)
        .max()
        .unwrap_or(1);
    let width = (content_width + CLIPBOARD_TEXT_PADDING * 2).max(160);
    let height = (line_height * lines.len() as u32)
        + (CLIPBOARD_TEXT_LINE_GAP * lines.len().saturating_sub(1) as u32)
        + CLIPBOARD_TEXT_PADDING * 2;
    let mut image = ImageBuffer::from_pixel(width, height, Rgba([248, 249, 250, 255]));

    let mut y = CLIPBOARD_TEXT_PADDING as i32;
    for line in lines {
        draw_text_mut(
            &mut image,
            Rgba([28, 32, 36, 255]),
            CLIPBOARD_TEXT_PADDING as i32,
            y,
            scale,
            &font,
            &line,
        );
        y += (line_height + CLIPBOARD_TEXT_LINE_GAP) as i32;
    }

    Ok(image)
}

fn truncate_clipboard_text(text: &str) -> String {
    let mut output = text
        .chars()
        .take(CLIPBOARD_TEXT_MAX_CHARS)
        .collect::<String>();
    if text.chars().count() > CLIPBOARD_TEXT_MAX_CHARS {
        output.push_str("\n...");
    }
    output
}

fn wrap_text_lines(font: &FontArc, scale: PxScale, text: &str, max_width: u32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for character in paragraph.chars() {
            let mut candidate = current.clone();
            candidate.push(character);
            if !current.is_empty() && text_size(scale, font, &candidate).0 > max_width {
                lines.push(current);
                current = character.to_string();
            } else {
                current = candidate;
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }
    lines
}

fn load_clipboard_pin_font() -> Option<FontArc> {
    [
        r"C:\Windows\Fonts\Deng.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
    ]
    .iter()
    .filter_map(|path| std::fs::read(path).ok())
    .find_map(|bytes| FontArc::try_from_vec(bytes).ok())
}

fn spawn_pin_window_from_desktop(
    app: &AppHandle,
    launch: &PinWindowLaunch<'_>,
) -> Result<(), String> {
    let args = vec![
        "--pin".to_owned(),
        "--image".to_owned(),
        launch.image_path.to_string_lossy().into_owned(),
        "--language".to_owned(),
        launch.language.to_owned(),
        "--x".to_owned(),
        launch.x.to_string(),
        "--y".to_owned(),
        launch.y.to_string(),
        "--width".to_owned(),
        launch.width.to_string(),
        "--height".to_owned(),
        launch.height.to_string(),
        "--pin-opacity".to_owned(),
        launch.opacity.to_string(),
        "--pin-zoom-step".to_owned(),
        launch.zoom_step.to_string(),
        "--pin-min-width".to_owned(),
        launch.min_width.to_string(),
        "--pin-min-height".to_owned(),
        launch.min_height.to_string(),
        "--pin-always-on-top".to_owned(),
        launch.always_on_top.to_string(),
        "--ocr-provider".to_owned(),
        launch.ocr_provider.to_owned(),
        "--ocr-language-hint".to_owned(),
        launch.ocr_language_hint.unwrap_or("").to_owned(),
        "--ocr-default-model-id".to_owned(),
        launch.ocr_default_model_id.unwrap_or("").to_owned(),
        "--ocr-models-registry".to_owned(),
        launch.ocr_models_registry.unwrap_or("").to_owned(),
        "--translate-provider".to_owned(),
        launch.translate_provider.to_owned(),
        "--translate-target-language".to_owned(),
        launch.translate_target_language.to_owned(),
        "--translate-segmentation-mode".to_owned(),
        launch.translate_segmentation_mode.to_owned(),
        "--translate-default-model-id".to_owned(),
        launch.translate_default_model_id.unwrap_or("").to_owned(),
        "--smart-merge-edge-tolerance-lines".to_owned(),
        launch.smart_merge_edge_tolerance_lines.to_string(),
        "--smart-merge-loose-edge-tolerance-lines".to_owned(),
        launch.smart_merge_loose_edge_tolerance_lines.to_string(),
        "--smart-merge-height-ratio-limit".to_owned(),
        launch.smart_merge_height_ratio_limit.to_string(),
        "--smart-merge-longer-line-ratio".to_owned(),
        launch.smart_merge_longer_line_ratio.to_string(),
        "--smart-merge-short-last-line-ratio".to_owned(),
        launch.smart_merge_short_last_line_ratio.to_string(),
        "--smart-merge-inline-label-max-chars".to_owned(),
        launch.smart_merge_inline_label_max_chars.to_string(),
        "--ocr-text-font-height-ratio".to_owned(),
        launch.ocr_text_font_height_ratio.to_string(),
        "--ocr-text-min-font-size".to_owned(),
        launch.ocr_text_min_font_size.to_string(),
        "--ocr-text-max-font-size".to_owned(),
        launch.ocr_text_max_font_size.to_string(),
        "--ocr-text-padding-x".to_owned(),
        launch.ocr_text_padding_x.to_string(),
        "--ocr-text-padding-y".to_owned(),
        launch.ocr_text_padding_y.to_string(),
        "--ocr-text-interaction-padding-x".to_owned(),
        launch.ocr_text_interaction_padding_x.to_string(),
        "--ocr-text-interaction-padding-y".to_owned(),
        launch.ocr_text_interaction_padding_y.to_string(),
        "--owner-pid".to_owned(),
        std::process::id().to_string(),
    ];
    let overlay = overlay_launch(app)?;
    log::info!(
        "spawning clipboard pin window via {} image={} size={}x{}",
        overlay.description(),
        launch.image_path.display(),
        launch.width,
        launch.height
    );
    let mut command = overlay.command(args);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command
        .spawn()
        .map_err(|error| format!("failed to launch clipboard pin window: {error}"))?;
    log::info!("clipboard pin window spawned pid={}", child.id());
    app.state::<Mutex<CaptureOverlayRuntime>>()
        .lock()
        .map_err(|_| "overlay runtime lock poisoned".to_owned())?
        .track_pin_process(child);
    Ok(())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn control_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], OVERLAY_CONTROL_PORT))
}

fn resident_overlay_args(
    settings: &Settings,
    model_registry_path: Option<&str>,
    owner_pid: u32,
) -> Vec<String> {
    vec![
        "--capture".to_owned(),
        "--resident".to_owned(),
        "--control-port".to_owned(),
        OVERLAY_CONTROL_PORT.to_string(),
        "--owner-pid".to_owned(),
        owner_pid.to_string(),
        "--language".to_owned(),
        settings.interface.language.clone(),
        "--mask-opacity".to_owned(),
        settings.capture.mask_opacity.to_string(),
        "--border-color".to_owned(),
        settings.capture.border_color.clone(),
        "--show-size-label".to_owned(),
        settings.capture.show_size_label.to_string(),
        "--show-toolbar".to_owned(),
        settings.capture.show_toolbar.to_string(),
        "--show-magnifier".to_owned(),
        settings.overlay.show_magnifier.to_string(),
        "--magnifier-scale".to_owned(),
        settings.overlay.magnifier_scale.to_string(),
        "--pin-hotkey".to_owned(),
        settings.hotkeys.pin_selection.clone(),
        "--completion-action".to_owned(),
        completion_action_name(&settings.capture.completion_action).to_owned(),
        "--pin-opacity".to_owned(),
        settings.pin.default_opacity.to_string(),
        "--pin-zoom-step".to_owned(),
        settings.pin.zoom_step.to_string(),
        "--pin-min-width".to_owned(),
        settings.pin.min_width.to_string(),
        "--pin-min-height".to_owned(),
        settings.pin.min_height.to_string(),
        "--pin-always-on-top".to_owned(),
        settings.pin.always_on_top.to_string(),
        "--ocr-provider".to_owned(),
        ocr_provider_name(&settings.ocr.provider),
        "--ocr-language-hint".to_owned(),
        settings.ocr.language_hint.clone().unwrap_or_default(),
        "--ocr-default-model-id".to_owned(),
        settings.ocr.default_model_id.clone().unwrap_or_default(),
        "--ocr-models-registry".to_owned(),
        model_registry_path.unwrap_or("").to_owned(),
        "--translate-provider".to_owned(),
        translate_provider_name(&settings.translate.provider),
        "--translate-target-language".to_owned(),
        settings.translate.target_language.clone(),
        "--translate-segmentation-mode".to_owned(),
        settings.translate.segmentation_mode.clone(),
        "--translate-default-model-id".to_owned(),
        settings
            .translate
            .default_model_id
            .clone()
            .unwrap_or_default(),
        "--smart-merge-edge-tolerance-lines".to_owned(),
        settings
            .translate
            .smart_merge
            .edge_tolerance_lines
            .to_string(),
        "--smart-merge-loose-edge-tolerance-lines".to_owned(),
        settings
            .translate
            .smart_merge
            .loose_edge_tolerance_lines
            .to_string(),
        "--smart-merge-height-ratio-limit".to_owned(),
        settings
            .translate
            .smart_merge
            .height_ratio_limit
            .to_string(),
        "--smart-merge-longer-line-ratio".to_owned(),
        settings.translate.smart_merge.longer_line_ratio.to_string(),
        "--smart-merge-short-last-line-ratio".to_owned(),
        settings
            .translate
            .smart_merge
            .short_last_line_ratio
            .to_string(),
        "--smart-merge-inline-label-max-chars".to_owned(),
        settings
            .translate
            .smart_merge
            .inline_label_max_chars
            .to_string(),
        "--ocr-text-font-height-ratio".to_owned(),
        settings.pin.ocr_text.font_height_ratio.to_string(),
        "--ocr-text-min-font-size".to_owned(),
        settings.pin.ocr_text.min_font_size.to_string(),
        "--ocr-text-max-font-size".to_owned(),
        settings.pin.ocr_text.max_font_size.to_string(),
        "--ocr-text-padding-x".to_owned(),
        settings.pin.ocr_text.padding_x.to_string(),
        "--ocr-text-padding-y".to_owned(),
        settings.pin.ocr_text.padding_y.to_string(),
        "--ocr-text-interaction-padding-x".to_owned(),
        settings.pin.ocr_text.interaction_padding_x.to_string(),
        "--ocr-text-interaction-padding-y".to_owned(),
        settings.pin.ocr_text.interaction_padding_y.to_string(),
    ]
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayCaptureCommand {
    kind: &'static str,
    snapshot: SharedSnapshotCommand,
    regions: Vec<CaptureRegionCommand>,
    language: String,
    mask_opacity: f32,
    border_color: String,
    show_size_label: bool,
    show_toolbar: bool,
    show_magnifier: bool,
    magnifier_scale: f32,
    pin_hotkey: String,
    completion_action: String,
    pin_opacity: f32,
    pin_zoom_step: f32,
    pin_min_width: f32,
    pin_min_height: f32,
    pin_always_on_top: bool,
    ocr_provider: String,
    ocr_language_hint: Option<String>,
    ocr_default_model_id: Option<String>,
    ocr_models_registry: Option<String>,
    translate_provider: String,
    translate_target_language: String,
    translate_segmentation_mode: String,
    translate_default_model_id: Option<String>,
    smart_merge_edge_tolerance_lines: f32,
    smart_merge_loose_edge_tolerance_lines: f32,
    smart_merge_height_ratio_limit: f32,
    smart_merge_longer_line_ratio: f32,
    smart_merge_short_last_line_ratio: f32,
    smart_merge_inline_label_max_chars: usize,
    ocr_text_font_height_ratio: f32,
    ocr_text_min_font_size: f32,
    ocr_text_max_font_size: f32,
    ocr_text_padding_x: f32,
    ocr_text_padding_y: f32,
    ocr_text_interaction_padding_x: f32,
    ocr_text_interaction_padding_y: f32,
}

impl OverlayCaptureCommand {
    fn from_settings(settings: &Settings, snapshot: &SnapshotCapture) -> Self {
        Self {
            kind: "capture",
            snapshot: SharedSnapshotCommand {
                mapping_name: snapshot.mapping_name.clone(),
                byte_len: snapshot.byte_len,
                width: snapshot.width,
                height: snapshot.height,
                format: image_format_name(snapshot.format).to_owned(),
                origin_x: snapshot.bounds.origin.x,
                origin_y: snapshot.bounds.origin.y,
            },
            regions: snapshot
                .regions
                .iter()
                .map(CaptureRegionCommand::from_region)
                .collect(),
            language: settings.interface.language.clone(),
            mask_opacity: settings.capture.mask_opacity,
            border_color: settings.capture.border_color.clone(),
            show_size_label: settings.capture.show_size_label,
            show_toolbar: settings.capture.show_toolbar,
            show_magnifier: settings.overlay.show_magnifier,
            magnifier_scale: settings.overlay.magnifier_scale,
            pin_hotkey: settings.hotkeys.pin_selection.clone(),
            completion_action: completion_action_name(&settings.capture.completion_action)
                .to_owned(),
            pin_opacity: settings.pin.default_opacity,
            pin_zoom_step: settings.pin.zoom_step,
            pin_min_width: settings.pin.min_width,
            pin_min_height: settings.pin.min_height,
            pin_always_on_top: settings.pin.always_on_top,
            ocr_provider: ocr_provider_name(&settings.ocr.provider),
            ocr_language_hint: settings.ocr.language_hint.clone(),
            ocr_default_model_id: settings.ocr.default_model_id.clone(),
            ocr_models_registry: None,
            translate_provider: translate_provider_name(&settings.translate.provider),
            translate_target_language: settings.translate.target_language.clone(),
            translate_segmentation_mode: settings.translate.segmentation_mode.clone(),
            translate_default_model_id: settings.translate.default_model_id.clone(),
            smart_merge_edge_tolerance_lines: settings.translate.smart_merge.edge_tolerance_lines,
            smart_merge_loose_edge_tolerance_lines: settings
                .translate
                .smart_merge
                .loose_edge_tolerance_lines,
            smart_merge_height_ratio_limit: settings.translate.smart_merge.height_ratio_limit,
            smart_merge_longer_line_ratio: settings.translate.smart_merge.longer_line_ratio,
            smart_merge_short_last_line_ratio: settings.translate.smart_merge.short_last_line_ratio,
            smart_merge_inline_label_max_chars: settings
                .translate
                .smart_merge
                .inline_label_max_chars,
            ocr_text_font_height_ratio: settings.pin.ocr_text.font_height_ratio,
            ocr_text_min_font_size: settings.pin.ocr_text.min_font_size,
            ocr_text_max_font_size: settings.pin.ocr_text.max_font_size,
            ocr_text_padding_x: settings.pin.ocr_text.padding_x,
            ocr_text_padding_y: settings.pin.ocr_text.padding_y,
            ocr_text_interaction_padding_x: settings.pin.ocr_text.interaction_padding_x,
            ocr_text_interaction_padding_y: settings.pin.ocr_text.interaction_padding_y,
        }
    }

    fn with_model_registry_path(mut self, path: Option<String>) -> Self {
        self.ocr_models_registry = path;
        self
    }
}

fn ocr_provider_name(provider: &OcrProvider) -> String {
    match provider {
        OcrProvider::Disabled => "disabled",
        OcrProvider::System => "system",
        OcrProvider::Local(OcrLocalBackend::Mnn) => "local-mnn",
        OcrProvider::Local(OcrLocalBackend::OnnxRuntime) => "local-onnx",
        OcrProvider::Local(OcrLocalBackend::PaddleRuntime) => "local-paddle",
        OcrProvider::Local(OcrLocalBackend::Custom(_)) => "local-custom",
        OcrProvider::ExternalApi(OcrExternalProvider::OpenAi) => "api-openai",
        OcrProvider::ExternalApi(OcrExternalProvider::AzureVision) => "api-azure",
        OcrProvider::ExternalApi(OcrExternalProvider::GoogleVision) => "api-google",
        OcrProvider::ExternalApi(OcrExternalProvider::BaiduOcr) => "api-baidu",
        OcrProvider::ExternalApi(OcrExternalProvider::TencentOcr) => "api-tencent",
        OcrProvider::ExternalApi(OcrExternalProvider::Custom(_)) => "api-custom",
    }
    .to_owned()
}

fn translate_provider_name(provider: &TranslateProvider) -> String {
    match provider {
        TranslateProvider::Disabled => "disabled",
        TranslateProvider::Local(TranslateLocalBackend::CTranslate2) => "local-ct2",
        TranslateProvider::Local(TranslateLocalBackend::Custom(_)) => "local-custom",
        TranslateProvider::ExternalApi(TranslateExternalProvider::DeepL) => "api-deepl",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Google) => "api-google",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Azure) => "api-azure",
        TranslateProvider::ExternalApi(TranslateExternalProvider::OpenAi) => "api-openai",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Baidu) => "api-baidu",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Tencent) => "api-tencent",
        TranslateProvider::ExternalApi(TranslateExternalProvider::CustomHttp) => "api-custom",
        TranslateProvider::ExternalApi(TranslateExternalProvider::Custom(_)) => "api-custom",
        TranslateProvider::Experimental(_) => "experimental",
        TranslateProvider::Custom(_) => "custom",
    }
    .to_owned()
}

fn completion_action_name(action: &CaptureCompletionAction) -> &'static str {
    match action {
        CaptureCompletionAction::Pin => "pin",
        CaptureCompletionAction::CopyToClipboard => "copy",
        CaptureCompletionAction::SaveToFile => "save",
        CaptureCompletionAction::OpenEditor => "editor",
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureRegionCommand {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    depth: u8,
}

impl CaptureRegionCommand {
    fn from_region(region: &CaptureWindowRegion) -> Self {
        Self {
            x: region.bounds.origin.x,
            y: region.bounds.origin.y,
            width: region.bounds.size.width,
            height: region.bounds.size.height,
            depth: region.depth,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SharedSnapshotCommand {
    mapping_name: String,
    byte_len: usize,
    width: u32,
    height: u32,
    format: String,
    origin_x: f32,
    origin_y: f32,
}

fn image_format_name(format: shared_models::ImageFormat) -> &'static str {
    match format {
        shared_models::ImageFormat::Rgba8 => "rgba8",
        shared_models::ImageFormat::Bgra8 => "bgra8",
        shared_models::ImageFormat::Png => "png",
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayControlResponse {
    kind: String,
    message: Option<String>,
}
