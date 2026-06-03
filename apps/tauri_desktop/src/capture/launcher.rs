use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use platform_win32::{
    CaptureWindowRegion, HotkeyListener, HotkeyRegistration, NamedSharedMemory, listen_for_hotkey,
};
use serde::{Deserialize, Serialize};
use shared_models::{
    CaptureCompletionAction, OcrExternalProvider, OcrLocalBackend, OcrProvider, Settings,
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
const RECENT_MAPPING_LIMIT: usize = 4;

#[derive(Default)]
pub struct CaptureOverlayRuntime {
    process: Option<Child>,
    recent_mappings: VecDeque<NamedSharedMemory>,
}

impl CaptureOverlayRuntime {
    fn keep_mapping(&mut self, mapping: NamedSharedMemory) {
        self.recent_mappings.push_back(mapping);
        while self.recent_mappings.len() > RECENT_MAPPING_LIMIT {
            self.recent_mappings.pop_front();
        }
    }
}

impl Drop for CaptureOverlayRuntime {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            log::info!("stopping resident overlay process pid={}", process.id());
            let _ = process.kill();
            let _ = process.wait();
        }
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
    log::info!(
        "launching capture overlay include_cursor={} language={}",
        settings.capture.include_cursor,
        settings.interface.language
    );
    ensure_overlay_resident_for_settings(app, settings)?;

    if settings.capture.capture_delay_ms > 0 {
        thread::sleep(Duration::from_millis(settings.capture.capture_delay_ms));
    }

    let model_registry_path = models::models_path(app)
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    let snapshot = capture_snapshot(settings.capture.include_cursor)?;
    let command = OverlayCaptureCommand::from_settings(settings, &snapshot)
        .with_model_registry_path(model_registry_path.clone());
    let managed = app.state::<Mutex<CaptureOverlayRuntime>>();
    let mut runtime = managed
        .lock()
        .map_err(|_| "overlay runtime lock poisoned".to_owned())?;

    if let Err(first_error) = send_capture_command(&command) {
        log::warn!("capture command failed; restarting resident overlay: {first_error}");
        ensure_overlay_resident_locked(app, settings, &mut runtime)?;
        send_capture_command(&command)
            .map_err(|second_error| format!("{first_error}; retry failed: {second_error}"))?;
    }

    runtime.keep_mapping(snapshot.mapping);
    log::info!(
        "capture overlay launched snapshot={}x{} bytes={}",
        command.snapshot.width,
        command.snapshot.height,
        command.snapshot.byte_len
    );

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
    let managed = app.state::<Mutex<Option<HotkeyListener>>>();
    let mut guard = managed
        .lock()
        .map_err(|_| "hotkey listener lock poisoned".to_owned())?;
    *guard = None;

    let listener = listen_for_hotkey(registration, move |_| {
        log::info!("capture hotkey triggered");
        if let Err(error) = launch_capture_overlay(&app_handle) {
            log::error!("failed to launch capture overlay from hotkey: {error}");
        }
    })
    .map_err(|error| error.to_string())?;

    *guard = Some(listener);
    log::info!("capture hotkey registered");

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
    if overlay_server_ready() {
        log::info!("resident overlay server already ready");
        return Ok(());
    }

    if let Some(process) = runtime.process.as_mut() {
        if process
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            log::warn!("tracked resident overlay process exited");
            runtime.process = None;
        }
    }

    if let Some(mut process) = runtime.process.take() {
        log::info!("restarting resident overlay process pid={}", process.id());
        let _ = process.kill();
        let _ = process.wait();
    }

    if runtime.process.is_none() {
        let launch = overlay_launch(app)?;
        let model_registry_path = models::models_path(app)
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        let args = resident_overlay_args(settings, model_registry_path.as_deref());
        log::info!("starting resident overlay via {}", launch.description());
        let mut command = launch.command(args);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command
            .spawn()
            .map_err(|error| format!("failed to launch resident screenshot overlay: {error}"))?;
        log::info!("resident overlay process started pid={}", child.id());
        runtime.process = Some(child);
    }

    wait_for_overlay_server(runtime.process.as_mut(), OVERLAY_READY_TIMEOUT)
}

fn overlay_server_ready() -> bool {
    let Ok(mut stream) = TcpStream::connect(control_addr()) else {
        return false;
    };

    let _ = stream.set_read_timeout(Some(Duration::from_millis(120)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(120)));
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
    let mut stream = TcpStream::connect(control_addr())
        .map_err(|error| format!("failed to connect to resident screenshot overlay: {error}"))?;
    let _ = stream.set_write_timeout(Some(OVERLAY_COMMAND_TIMEOUT));
    let _ = stream.set_read_timeout(Some(OVERLAY_COMMAND_TIMEOUT));
    stream
        .write_all(json.as_bytes())
        .and_then(|_| stream.write_all(b"\n"))
        .and_then(|_| stream.flush())
        .map_err(|error| format!("failed to send screenshot command: {error}"))?;

    let mut response = String::new();
    let count = BufReader::new(stream)
        .read_line(&mut response)
        .map_err(|error| format!("failed to read screenshot command response: {error}"))?;
    if count == 0 {
        return Err("resident screenshot overlay closed the command connection".to_owned());
    }

    match serde_json::from_str::<OverlayControlResponse>(&response) {
        Ok(response) if response.kind == "accepted" => {
            log::info!("resident overlay accepted capture command");
            Ok(())
        }
        Ok(response) if response.kind == "error" => {
            let message = response
                .message
                .unwrap_or_else(|| "resident screenshot overlay rejected the command".to_owned());
            log::error!("resident overlay rejected capture command: {message}");
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

fn control_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], OVERLAY_CONTROL_PORT))
}

fn resident_overlay_args(settings: &Settings, model_registry_path: Option<&str>) -> Vec<String> {
    vec![
        "--capture".to_owned(),
        "--resident".to_owned(),
        "--control-port".to_owned(),
        OVERLAY_CONTROL_PORT.to_string(),
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
    pin_always_on_top: bool,
    ocr_provider: String,
    ocr_language_hint: Option<String>,
    ocr_default_model_id: Option<String>,
    ocr_models_registry: Option<String>,
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
            pin_always_on_top: settings.pin.always_on_top,
            ocr_provider: ocr_provider_name(&settings.ocr.provider),
            ocr_language_hint: settings.ocr.language_hint.clone(),
            ocr_default_model_id: settings.ocr.default_model_id.clone(),
            ocr_models_registry: None,
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
    origin_x: f32,
    origin_y: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayControlResponse {
    kind: String,
    message: Option<String>,
}
