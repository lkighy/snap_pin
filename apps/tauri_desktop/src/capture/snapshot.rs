use std::time::{SystemTime, UNIX_EPOCH};

use platform_win32::{
    CaptureRequest, CaptureWindowRegion, DxgiCaptureBackend, GdiCaptureBackend, NamedSharedMemory,
    WgcCaptureBackend, WindowsCaptureBackend, capture_window_regions, create_named_shared_memory,
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
    pub(crate) mapping: NamedSharedMemory,
}

pub(crate) fn capture_snapshot(include_cursor: bool) -> Result<SnapshotCapture, String> {
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
