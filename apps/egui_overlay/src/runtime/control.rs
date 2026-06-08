use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Sender},
};
use std::thread;
use std::time::Duration;

use eframe::egui::Context;
use serde::Deserialize;

use crate::capture::snapshot_io::CaptureRegionCommand;

pub(crate) const CONTROL_PROTOCOL_VERSION: u32 = 2;
const COMMAND_ACK_TIMEOUT_MS: u64 = 5_000;

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

fn default_pin_min_width() -> f32 {
    96.0
}

fn default_pin_min_height() -> f32 {
    72.0
}

fn default_ocr_provider() -> String {
    "local-mnn".to_owned()
}

fn default_snapshot_format() -> String {
    "rgba8".to_owned()
}

fn default_translate_provider() -> String {
    "local-ct2".to_owned()
}

fn default_translate_target_language() -> String {
    "zh-CN".to_owned()
}

fn default_translate_segmentation_mode() -> String {
    "smart-merge".to_owned()
}

fn default_smart_merge_edge_tolerance_lines() -> f32 {
    1.35
}

fn default_smart_merge_loose_edge_tolerance_lines() -> f32 {
    2.4
}

fn default_smart_merge_height_ratio_limit() -> f32 {
    1.5
}

fn default_smart_merge_longer_line_ratio() -> f32 {
    1.35
}

fn default_smart_merge_short_last_line_ratio() -> f32 {
    0.72
}

fn default_smart_merge_inline_label_max_chars() -> usize {
    32
}

fn default_ocr_text_font_height_ratio() -> f32 {
    0.46
}

fn default_ocr_text_min_font_size() -> f32 {
    6.0
}

fn default_ocr_text_max_font_size() -> f32 {
    42.0
}

fn default_ocr_text_padding_x() -> f32 {
    2.0
}

fn default_ocr_text_padding_y() -> f32 {
    1.0
}

fn default_ocr_text_interaction_padding_x() -> f32 {
    2.0
}

fn default_ocr_text_interaction_padding_y() -> f32 {
    4.0
}

#[derive(Debug)]
pub(crate) enum OverlayCommand {
    Capture(OverlayCaptureCommand),
    PinSelection,
    Shutdown,
    Error(String),
}

pub(crate) type OverlayCommandQueue = Arc<Mutex<VecDeque<QueuedOverlayCommand>>>;

pub(crate) fn new_overlay_command_queue() -> OverlayCommandQueue {
    Arc::new(Mutex::new(VecDeque::new()))
}

#[derive(Debug)]
pub(crate) struct QueuedOverlayCommand {
    pub(crate) command: OverlayCommand,
    pub(crate) completion: Option<Sender<Result<(), String>>>,
}

impl QueuedOverlayCommand {
    pub(crate) fn new(command: OverlayCommand) -> Self {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlayCaptureCommand {
    pub(crate) kind: String,
    pub(crate) snapshot: SharedSnapshotCommand,
    pub(crate) regions: Vec<CaptureRegionCommand>,
    pub(crate) language: String,
    pub(crate) mask_opacity: f32,
    pub(crate) border_color: String,
    #[serde(default = "default_true")]
    pub(crate) show_size_label: bool,
    #[serde(default = "default_true")]
    pub(crate) show_toolbar: bool,
    #[serde(default = "default_show_magnifier")]
    pub(crate) show_magnifier: bool,
    #[serde(default = "default_magnifier_scale")]
    pub(crate) magnifier_scale: f32,
    #[serde(default = "default_pin_hotkey")]
    pub(crate) pin_hotkey: String,
    #[serde(default = "default_completion_action")]
    pub(crate) completion_action: String,
    #[serde(default = "default_pin_opacity")]
    pub(crate) pin_opacity: f32,
    #[serde(default = "default_pin_zoom_step")]
    pub(crate) pin_zoom_step: f32,
    #[serde(default = "default_pin_min_width")]
    pub(crate) pin_min_width: f32,
    #[serde(default = "default_pin_min_height")]
    pub(crate) pin_min_height: f32,
    #[serde(default = "default_true")]
    pub(crate) pin_always_on_top: bool,
    #[serde(default = "default_ocr_provider")]
    pub(crate) ocr_provider: String,
    #[serde(default)]
    pub(crate) ocr_language_hint: Option<String>,
    #[serde(default)]
    pub(crate) ocr_default_model_id: Option<String>,
    #[serde(default)]
    pub(crate) ocr_models_registry: Option<String>,
    #[serde(default = "default_translate_provider")]
    pub(crate) translate_provider: String,
    #[serde(default = "default_translate_target_language")]
    pub(crate) translate_target_language: String,
    #[serde(default = "default_translate_segmentation_mode")]
    pub(crate) translate_segmentation_mode: String,
    #[serde(default)]
    pub(crate) translate_default_model_id: Option<String>,
    #[serde(default = "default_smart_merge_edge_tolerance_lines")]
    pub(crate) smart_merge_edge_tolerance_lines: f32,
    #[serde(default = "default_smart_merge_loose_edge_tolerance_lines")]
    pub(crate) smart_merge_loose_edge_tolerance_lines: f32,
    #[serde(default = "default_smart_merge_height_ratio_limit")]
    pub(crate) smart_merge_height_ratio_limit: f32,
    #[serde(default = "default_smart_merge_longer_line_ratio")]
    pub(crate) smart_merge_longer_line_ratio: f32,
    #[serde(default = "default_smart_merge_short_last_line_ratio")]
    pub(crate) smart_merge_short_last_line_ratio: f32,
    #[serde(default = "default_smart_merge_inline_label_max_chars")]
    pub(crate) smart_merge_inline_label_max_chars: usize,
    #[serde(default = "default_ocr_text_font_height_ratio")]
    pub(crate) ocr_text_font_height_ratio: f32,
    #[serde(default = "default_ocr_text_min_font_size")]
    pub(crate) ocr_text_min_font_size: f32,
    #[serde(default = "default_ocr_text_max_font_size")]
    pub(crate) ocr_text_max_font_size: f32,
    #[serde(default = "default_ocr_text_padding_x")]
    pub(crate) ocr_text_padding_x: f32,
    #[serde(default = "default_ocr_text_padding_y")]
    pub(crate) ocr_text_padding_y: f32,
    #[serde(default = "default_ocr_text_interaction_padding_x")]
    pub(crate) ocr_text_interaction_padding_x: f32,
    #[serde(default = "default_ocr_text_interaction_padding_y")]
    pub(crate) ocr_text_interaction_padding_y: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlayPingCommand {
    kind: String,
    protocol: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlayPinSelectionCommand {
    kind: String,
    protocol: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlayShutdownCommand {
    kind: String,
    protocol: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedSnapshotCommand {
    pub(crate) mapping_name: String,
    pub(crate) byte_len: usize,
    pub(crate) width: u32,
    pub(crate) height: u32,
    #[serde(default = "default_snapshot_format")]
    pub(crate) format: String,
    pub(crate) origin_x: f32,
    pub(crate) origin_y: f32,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayControlResponse {
    kind: &'static str,
    message: Option<String>,
}

pub(crate) fn start_control_server(port: u16, queue: OverlayCommandQueue, ctx: Context) {
    thread::spawn(move || {
        let listener = match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => listener,
            Err(error) => {
                log::error!("failed to bind overlay control port {port}: {error}");
                let _ = push_overlay_command(
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
                                let result = queue_overlay_command_with_ack(
                                    &queue,
                                    &ctx,
                                    OverlayCommand::PinSelection,
                                    "pin-selection",
                                );
                                write_control_response(&mut stream, result);
                            } else if is_supported_shutdown_command(&line) {
                                log::info!("overlay shutdown command accepted by control thread");
                                let result = queue_overlay_command_with_ack(
                                    &queue,
                                    &ctx,
                                    OverlayCommand::Shutdown,
                                    "shutdown",
                                );
                                write_control_response(&mut stream, result);
                            } else {
                                match serde_json::from_str::<OverlayCaptureCommand>(&line) {
                                    Ok(command) => {
                                        log::info!(
                                            "overlay capture command accepted by control thread"
                                        );
                                        let result = queue_overlay_command(
                                            &queue,
                                            &ctx,
                                            OverlayCommand::Capture(command),
                                            "capture",
                                        );
                                        write_control_response(&mut stream, result);
                                    }
                                    Err(error) => {
                                        let message =
                                            format!("failed to parse overlay command: {error}");
                                        let _ = push_overlay_command(
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
                            let _ = push_overlay_command(
                                &queue,
                                QueuedOverlayCommand::new(OverlayCommand::Error(format!(
                                    "failed to read overlay command: {error}"
                                ))),
                            );
                        }
                    }
                }
                Err(error) => {
                    log::error!("overlay control connection failed: {error}");
                    let _ = push_overlay_command(
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

fn queue_overlay_command_with_ack(
    queue: &OverlayCommandQueue,
    ctx: &Context,
    command: OverlayCommand,
    label: &'static str,
) -> Result<(), String> {
    let (completion_tx, completion_rx) = mpsc::channel();
    push_overlay_command(
        queue,
        QueuedOverlayCommand::with_completion(command, completion_tx),
    )?;
    ctx.request_repaint();

    let result = completion_rx
        .recv_timeout(Duration::from_millis(COMMAND_ACK_TIMEOUT_MS))
        .unwrap_or_else(|_| {
            log::error!(
                "overlay UI did not ACK {label} command within {} ms",
                COMMAND_ACK_TIMEOUT_MS
            );
            Err(format!(
                "resident screenshot overlay did not process the {label} command in time"
            ))
        });
    let ack_status = match &result {
        Ok(()) => "accepted",
        Err(message) if label == "pin-selection" && message == "capture_overlay_inactive" => {
            "inactive"
        }
        Err(_) => "rejected",
    };
    log::info!("overlay {label} command ACK status={ack_status}");
    result
}

fn queue_overlay_command(
    queue: &OverlayCommandQueue,
    ctx: &Context,
    command: OverlayCommand,
    label: &'static str,
) -> Result<(), String> {
    push_overlay_command(queue, QueuedOverlayCommand::new(command))?;
    ctx.request_repaint();
    log::info!("overlay {label} command queued without UI ACK");
    Ok(())
}

fn push_overlay_command(
    queue: &OverlayCommandQueue,
    command: QueuedOverlayCommand,
) -> Result<(), String> {
    if let Ok(mut queue) = queue.lock() {
        queue.push_back(command);
        log::info!("overlay command queued pending={}", queue.len());
        Ok(())
    } else {
        let message = "overlay command queue lock poisoned".to_owned();
        log::error!("{message}");
        Err(message)
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

fn is_supported_shutdown_command(line: &str) -> bool {
    serde_json::from_str::<OverlayShutdownCommand>(line).is_ok_and(|command| {
        command.kind == "shutdown" && command.protocol == CONTROL_PROTOCOL_VERSION
    })
}
