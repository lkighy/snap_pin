use image::{DynamicImage, ImageBuffer, Rgba};
use shared_models::{ImageData, ImageFormat};

use crate::OcrEngineError;

pub fn decode_image(image: &ImageData) -> Result<DynamicImage, OcrEngineError> {
    match image.metadata.format {
        ImageFormat::Png => image::load_from_memory(&image.bytes)
            .map_err(|error| OcrEngineError::new("ocr_image_decode_failed", error.to_string())),
        ImageFormat::Rgba8 => rgba_image(image.bytes.clone(), image),
        ImageFormat::Bgra8 => {
            let mut rgba = image.bytes.clone();
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            rgba_image(rgba, image)
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
