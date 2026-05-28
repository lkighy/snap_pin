use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use platform_win32::{
    CaptureRequest, CaptureWindowRegion, DxgiCaptureBackend, GdiCaptureBackend, HotkeyListener,
    HotkeyRegistration, NamedSharedMemory, WgcCaptureBackend, WindowsCaptureBackend,
    capture_window_regions, create_named_shared_memory, listen_for_hotkey,
};
use serde::{Deserialize, Serialize};
use shared_models::{ImageFormat, Settings};
use tauri::{AppHandle, Manager};

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

    let snapshot = capture_snapshot(settings.capture.include_cursor)?;
    let command = OverlayCaptureCommand::from_settings(settings, &snapshot);
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
        let args = resident_overlay_args(settings);
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

fn resident_overlay_args(settings: &Settings) -> Vec<String> {
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
        "--show-magnifier".to_owned(),
        settings.overlay.show_magnifier.to_string(),
        "--magnifier-scale".to_owned(),
        settings.overlay.magnifier_scale.to_string(),
    ]
}

#[derive(Debug)]
struct SnapshotCapture {
    mapping_name: String,
    byte_len: usize,
    width: u32,
    height: u32,
    bounds: shared_models::Rect,
    regions: Vec<CaptureWindowRegion>,
    mapping: NamedSharedMemory,
}

fn capture_snapshot(include_cursor: bool) -> Result<SnapshotCapture, String> {
    let bounds = platform_win32::virtual_screen_bounds();
    log::info!("capturing snapshot bounds={bounds:?} include_cursor={include_cursor}");
    let request = CaptureRequest {
        region: Some(bounds),
        include_cursor,
    };
    let regions = capture_window_regions(bounds);
    let frame = capture_with_preferred_backend(request)?;
    let rgba = frame_to_rgba(&frame)?;
    let byte_len = rgba.len();
    let mapping_name = snapshot_mapping_name();
    let mapping =
        create_named_shared_memory(&mapping_name, &rgba).map_err(|error| error.to_string())?;

    log::info!(
        "snapshot captured size={}x{} bytes={} regions={}",
        frame.pixel_size.width,
        frame.pixel_size.height,
        byte_len,
        regions.len()
    );

    Ok(SnapshotCapture {
        mapping_name,
        byte_len,
        width: frame.pixel_size.width.round().max(1.0) as u32,
        height: frame.pixel_size.height.round().max(1.0) as u32,
        bounds,
        regions,
        mapping,
    })
}

fn capture_with_preferred_backend(
    request: CaptureRequest,
) -> Result<platform_win32::CapturedFrame, String> {
    let backends: [(&str, &dyn WindowsCaptureBackend); 3] = [
        ("wgc", &WgcCaptureBackend),
        ("dxgi", &DxgiCaptureBackend),
        ("gdi", &GdiCaptureBackend),
    ];
    let mut last_error = None;

    for (name, backend) in backends {
        match backend.capture(request.clone()) {
            Ok(frame) => {
                log::info!("screenshot backend succeeded backend={name}");
                return Ok(frame);
            }
            Err(error) if error.code == "not_implemented" => {
                log::info!("screenshot backend not implemented backend={name}");
                last_error = Some(error.to_string());
            }
            Err(error) => {
                log::warn!("screenshot backend failed backend={name}: {error}");
                last_error = Some(error.to_string());
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "no screenshot backend is available".to_owned()))
}

fn frame_to_rgba(frame: &platform_win32::CapturedFrame) -> Result<Vec<u8>, String> {
    match frame.format {
        ImageFormat::Rgba8 => Ok(frame.bytes.clone()),
        ImageFormat::Bgra8 => Ok(bgra_to_rgba(&frame.bytes)),
        ImageFormat::Png => Err("PNG screenshot frames cannot be shared as raw memory".to_owned()),
    }
}

fn bgra_to_rgba(bytes: &[u8]) -> Vec<u8> {
    bytes
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], 255])
        .collect()
}

fn snapshot_mapping_name() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros())
        .unwrap_or_default();
    format!(
        "Local\\snap_pin_snapshot_{}_{}",
        std::process::id(),
        timestamp
    )
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
    show_magnifier: bool,
    magnifier_scale: f32,
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
            show_magnifier: settings.overlay.show_magnifier,
            magnifier_scale: settings.overlay.magnifier_scale,
        }
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

enum OverlayLaunch {
    Executable(PathBuf),
    Cargo { workspace: PathBuf },
}

impl OverlayLaunch {
    fn description(&self) -> String {
        match self {
            OverlayLaunch::Executable(path) => format!("executable {}", path.display()),
            OverlayLaunch::Cargo { workspace } => format!("cargo run in {}", workspace.display()),
        }
    }

    fn command(&self, overlay_args: Vec<String>) -> std::process::Command {
        match self {
            OverlayLaunch::Executable(path) => {
                let mut command = std::process::Command::new(path);
                command.args(overlay_args);
                command
            }
            OverlayLaunch::Cargo { workspace } => {
                let mut command = std::process::Command::new("cargo");
                command
                    .arg("run")
                    .arg("-p")
                    .arg("egui_overlay")
                    .arg("--")
                    .args(overlay_args)
                    .current_dir(workspace);
                command
            }
        }
    }
}

fn overlay_launch(app: &AppHandle) -> Result<OverlayLaunch, String> {
    let current_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let file_name = executable_name("egui_overlay");

    if let Some(workspace) = find_workspace_root(&current_exe) {
        if is_workspace_debug_executable(&current_exe, &workspace) {
            log::info!(
                "using cargo overlay launch current_exe={} workspace={}",
                current_exe.display(),
                workspace.display()
            );
            return Ok(OverlayLaunch::Cargo { workspace });
        }
    }

    for directory in candidate_directories(app, &current_exe) {
        let candidate = directory.join(&file_name);
        if candidate.exists() {
            log::info!("using overlay executable {}", candidate.display());
            return Ok(OverlayLaunch::Executable(candidate));
        }
    }

    if let Some(workspace) = find_workspace_root(&current_exe) {
        log::info!(
            "falling back to cargo overlay launch workspace={}",
            workspace.display()
        );
        return Ok(OverlayLaunch::Cargo { workspace });
    }

    Ok(OverlayLaunch::Executable(
        current_exe.with_file_name(file_name),
    ))
}

fn is_workspace_debug_executable(current_exe: &Path, workspace: &Path) -> bool {
    current_exe.starts_with(workspace.join("target").join("debug"))
}

fn candidate_directories(app: &AppHandle, current_exe: &PathBuf) -> Vec<PathBuf> {
    let mut directories = Vec::new();

    if let Some(parent) = current_exe.parent() {
        directories.push(parent.to_path_buf());
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        directories.push(resource_dir);
    }

    directories
}

fn executable_name(name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{name}.exe")
    }

    #[cfg(not(windows))]
    {
        name.to_owned()
    }
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.parent();
    while let Some(directory) = current {
        let manifest = directory.join("Cargo.toml");
        let apps_dir = directory.join("apps");
        if manifest.exists() && apps_dir.exists() {
            return Some(directory.to_path_buf());
        }

        current = directory.parent();
    }

    std::env::current_dir().ok().and_then(|cwd| {
        if cwd.join("Cargo.toml").exists() && cwd.join("apps").exists() {
            Some(cwd)
        } else {
            None
        }
    })
}
