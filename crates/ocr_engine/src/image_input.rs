use image::{DynamicImage, ImageBuffer, Rgba};
use perf_trace::{PerfSpan, log_elapsed};
use shared_models::{ImageData, ImageFormat};

use crate::OcrEngineError;

pub fn decode_image(image: &ImageData) -> Result<DynamicImage, OcrEngineError> {
    let span = PerfSpan::new("ocr_decode_image_total")
        .field("format", image_format_label(image.metadata.format))
        .field("bytes", image.bytes.len())
        .field("width", image.metadata.pixel_size.width.round().max(1.0))
        .field("height", image.metadata.pixel_size.height.round().max(1.0));
    match image.metadata.format {
        ImageFormat::Png => {
            let decode_start = std::time::Instant::now();
            let result = image::load_from_memory(&image.bytes)
                .map_err(|error| OcrEngineError::new("ocr_image_decode_failed", error.to_string()));
            log_elapsed("ocr_decode_png_memory", decode_start);
            if result.is_ok() {
                span.finish();
            }
            result
        }
        ImageFormat::Rgba8 => {
            let clone_start = std::time::Instant::now();
            let bytes = image.bytes.clone();
            log_elapsed("ocr_decode_rgba_clone", clone_start);
            let result = rgba_image(bytes, image);
            if result.is_ok() {
                span.finish();
            }
            result
        }
        ImageFormat::Bgra8 => {
            let convert_start = std::time::Instant::now();
            let mut rgba = image.bytes.clone();
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            log_elapsed("ocr_decode_bgra_to_rgba", convert_start);
            let result = rgba_image(rgba, image);
            if result.is_ok() {
                span.finish();
            }
            result
        }
    }
}

fn rgba_image(bytes: Vec<u8>, image: &ImageData) -> Result<DynamicImage, OcrEngineError> {
    let width = image.metadata.pixel_size.width.round().max(1.0) as u32;
    let height = image.metadata.pixel_size.height.round().max(1.0) as u32;
    let expected_len = width as usize * height as usize * 4;

    if bytes.len() != expected_len {
        return Err(OcrEngineError::new(
            "ocr_image_buffer_mismatch",
            format!(
                "image '{}' has {} bytes but OCR expected {} bytes for {}x{} RGBA",
                image.id.0,
                bytes.len(),
                expected_len,
                width,
                height
            ),
        ));
    }

    ImageBuffer::<Rgba<u8>, Vec<u8>>::from_vec(width, height, bytes)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| {
            OcrEngineError::new(
                "ocr_image_buffer_invalid",
                format!("image '{}' could not be converted to RGBA", image.id.0),
            )
        })
}

fn image_format_label(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Rgba8 => "rgba8",
        ImageFormat::Bgra8 => "bgra8",
    }
}

#[cfg(test)]
mod tests {
    use shared_models::{ImageData, ImageFormat, ImageId, ImageMetadata, Size};

    use super::decode_image;

    #[test]
    fn decodes_rgba_buffer() {
        let image = ImageData {
            id: ImageId::new("rgba"),
            metadata: ImageMetadata {
                id: ImageId::new("rgba"),
                pixel_size: Size::new(2.0, 1.0),
                format: ImageFormat::Rgba8,
                monitor_name: None,
            },
            bytes: vec![255, 0, 0, 255, 0, 255, 0, 255],
        };

        assert_eq!(decode_image(&image).unwrap().width(), 2);
    }

    #[test]
    fn rejects_mismatched_buffer_size() {
        let image = ImageData {
            id: ImageId::new("bad"),
            metadata: ImageMetadata {
                id: ImageId::new("bad"),
                pixel_size: Size::new(2.0, 1.0),
                format: ImageFormat::Rgba8,
                monitor_name: None,
            },
            bytes: vec![255, 0, 0, 255],
        };

        let error = decode_image(&image).unwrap_err();

        assert_eq!(error.code, "ocr_image_buffer_mismatch");
    }
}
