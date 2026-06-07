use std::time::{SystemTime, UNIX_EPOCH};

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
    pub(crate) bounds: shared_models::Rect,
    pub(crate) regions: Vec<CaptureWindowRegion>,
    pub(crate) mapping: SharedMemoryHandle,
}

pub(crate) fn capture_snapshot(include_cursor: bool) -> Result<SnapshotCapture, String> {
    let platform = platform_runtime::create_platform();
    let bounds = platform
        .screen_capture()
        .virtual_bounds()
        .map_err(|error| error.to_string())?;
    log::info!("capturing snapshot bounds={bounds:?} include_cursor={include_cursor}");
    let request = CaptureRequest {
        region: Some(bounds),
        include_cursor,
        backend_hint: None,
    };
    let regions = platform
        .window_ops()
        .capture_window_regions(bounds)
        .map_err(|error| error.to_string())?;
    let frame = platform
        .screen_capture()
        .capture(request)
        .map_err(|error| error.to_string())?;
    let rgba = frame_to_rgba(&frame)?;
    let byte_len = rgba.len();
    let mapping_name = snapshot_mapping_name();
    let mapping = platform
        .shared_memory()
        .create(SharedMemoryCreateRequest {
            name: mapping_name.clone(),
            bytes: rgba,
        })
        .map_err(|error| error.to_string())?;

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

fn frame_to_rgba(frame: &CapturedFrame) -> Result<Vec<u8>, String> {
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
