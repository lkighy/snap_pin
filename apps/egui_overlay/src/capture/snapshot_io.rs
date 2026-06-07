use std::borrow::Cow;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{
    ColorImage, Context, Pos2, Rect as EguiRect, TextureHandle, TextureOptions, Vec2,
};
use image::{DynamicImage, GenericImageView};
use serde::Deserialize;

use crate::runtime::control::SharedSnapshotCommand;
use crate::runtime::text::OverlayText;

const MIN_CAPTURE_REGION_SIZE: f32 = 4.0;

pub(crate) const SAVE_CANCELED_CODE: &str = "save_canceled";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureRegionCommand {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    depth: u8,
}

pub(crate) struct LoadedSharedSnapshot {
    pub(crate) image: DynamicImage,
    pub(crate) tiles: Vec<SnapshotTile>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CaptureRegion {
    pub(crate) rect: EguiRect,
    pub(crate) depth: u8,
}

pub(crate) struct SnapshotTile {
    pub(crate) texture: TextureHandle,
    pub(crate) rect: EguiRect,
}

pub(crate) struct CroppedSnapshot {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

pub(crate) fn build_capture_regions(
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

            (rect.width() >= MIN_CAPTURE_REGION_SIZE && rect.height() >= MIN_CAPTURE_REGION_SIZE)
                .then_some(CaptureRegion {
                    rect,
                    depth: region.depth,
                })
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

pub(crate) fn load_shared_snapshot(
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

    let bytes = platform_runtime::create_platform()
        .shared_memory()
        .open(&snapshot.mapping_name, snapshot.byte_len)
        .map_err(|error| format!("{}: {error}", text.snapshot_load_failed))?;
    let rgba = image::RgbaImage::from_raw(snapshot.width, snapshot.height, bytes)
        .ok_or_else(|| format!("{}: invalid RGBA buffer", text.snapshot_load_failed))?;
    let tiles = build_snapshot_tiles(ctx, &rgba);

    Ok(LoadedSharedSnapshot {
        image: DynamicImage::ImageRgba8(rgba),
        tiles,
    })
}

pub(crate) fn load_snapshot(
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

pub(crate) fn crop_snapshot_to_file(
    snapshot: &DynamicImage,
    selection: EguiRect,
    text: &OverlayText,
) -> Result<CroppedSnapshot, String> {
    let cropped = crop_snapshot(snapshot, selection);
    let width = cropped.width();
    let height = cropped.height();
    let image_path = std::env::temp_dir().join(capture_file_name());

    cropped
        .save(&image_path)
        .map_err(|error| format!("{}: {error}", text.crop_failed))?;
    Ok(CroppedSnapshot {
        path: image_path,
        width,
        height,
    })
}

pub(crate) fn save_snapshot_to_file(
    snapshot: &DynamicImage,
    selection: EguiRect,
    text: &OverlayText,
) -> Result<PathBuf, String> {
    let cropped = crop_snapshot(snapshot, selection);
    let default_name = capture_file_name();
    log::info!("prompting save path default_name={default_name}");
    let Some(image_path) = platform_runtime::create_platform()
        .file_dialog()
        .save_png_path(&default_name)
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

pub(crate) fn copy_snapshot_to_clipboard(
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

pub(crate) fn capture_file_name() -> String {
    format!(
        "snap_pin_capture_{}.png",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    )
}
