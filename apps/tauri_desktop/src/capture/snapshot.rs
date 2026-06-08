use std::time::{SystemTime, UNIX_EPOCH};

use perf_trace::{PerfSpan, log_elapsed};
use platform_api::{
    CaptureRequest, CaptureWindowRegion, CapturedFrame, SharedMemoryCreateRequest,
    SharedMemoryHandle,
};
use shared_models::ImageFormat;

// Owns the shared-memory mapping long enough for the resident overlay to read it.
#[derive(Debug)]
pub(crate) struct SnapshotCapture {
    pub(crate) mapping_name: String,
    pub(crate) byte_len: usize,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: ImageFormat,
    pub(crate) bounds: shared_models::Rect,
    pub(crate) regions: Vec<CaptureWindowRegion>,
    pub(crate) mapping: SharedMemoryHandle,
}

pub(crate) fn capture_snapshot(include_cursor: bool) -> Result<SnapshotCapture, String> {
    let mut span = PerfSpan::new("capture_snapshot_total").field("include_cursor", include_cursor);
    let platform_start = std::time::Instant::now();
    let platform = platform_runtime::create_platform();
    log_elapsed("capture_snapshot_create_platform", platform_start);
    let bounds_start = std::time::Instant::now();
    let bounds = platform
        .screen_capture()
        .virtual_bounds()
        .map_err(|error| error.to_string())?;
    log_elapsed("capture_snapshot_virtual_bounds", bounds_start);
    log::info!("capturing snapshot bounds={bounds:?} include_cursor={include_cursor}");
    let request = CaptureRequest {
        region: Some(bounds),
        include_cursor,
        backend_hint: None,
    };
    let regions_start = std::time::Instant::now();
    let regions = platform
        .window_ops()
        .capture_window_regions(bounds)
        .map_err(|error| error.to_string())?;
    log_elapsed("capture_snapshot_window_regions", regions_start);
    let capture_start = std::time::Instant::now();
    let frame = platform
        .screen_capture()
        .capture(request)
        .map_err(|error| error.to_string())?;
    log_elapsed("capture_snapshot_backend_capture", capture_start);
    let width = frame.pixel_size.width.round().max(1.0) as u32;
    let height = frame.pixel_size.height.round().max(1.0) as u32;
    let raw_start = std::time::Instant::now();
    let (bytes, format) = frame_to_shared_bytes(frame)?;
    log_elapsed("capture_snapshot_frame_to_shared_bytes", raw_start);
    let byte_len = bytes.len();
    let mapping_name = snapshot_mapping_name();
    let shared_memory_start = std::time::Instant::now();
    let mapping = platform
        .shared_memory()
        .create(SharedMemoryCreateRequest {
            name: mapping_name.clone(),
            bytes,
        })
        .map_err(|error| error.to_string())?;
    log_elapsed("capture_snapshot_shared_memory_create", shared_memory_start);

    log::info!(
        "snapshot captured size={}x{} bytes={} regions={}",
        width,
        height,
        byte_len,
        regions.len()
    );
    span.add_field("width", width);
    span.add_field("height", height);
    span.add_field("format", image_format_name(format));
    span.add_field("bytes", byte_len);
    span.add_field("regions", regions.len());
    span.finish();

    Ok(SnapshotCapture {
        mapping_name,
        byte_len,
        width,
        height,
        format,
        bounds,
        regions,
        mapping,
    })
}

fn frame_to_shared_bytes(frame: CapturedFrame) -> Result<(Vec<u8>, ImageFormat), String> {
    match frame.format {
        ImageFormat::Rgba8 | ImageFormat::Bgra8 => Ok((frame.bytes, frame.format)),
        ImageFormat::Png => Err("PNG screenshot frames cannot be shared as raw memory".to_owned()),
    }
}

fn image_format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Rgba8 => "rgba8",
        ImageFormat::Bgra8 => "bgra8",
        ImageFormat::Png => "png",
    }
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
