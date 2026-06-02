use shared_models::{ImageData, OcrJob, OcrResult};

use crate::PlatformError;

#[cfg(windows)]
pub fn recognize_system_ocr(job: &OcrJob, image: &ImageData) -> Result<OcrResult, PlatformError> {
    imp::recognize_system_ocr(job, image)
}

#[cfg(not(windows))]
pub fn recognize_system_ocr(_job: &OcrJob, _image: &ImageData) -> Result<OcrResult, PlatformError> {
    Err(PlatformError::new(
        "unsupported_platform",
        "system OCR is currently implemented only on Windows",
    ))
}

#[cfg(windows)]
mod imp {
    use image::codecs::png::PngEncoder;
    use image::imageops::FilterType;
    use image::{ColorType, ImageEncoder, RgbaImage};
    use shared_models::{
        ImageData, ImageFormat, OcrJob, OcrResult, OcrTextBlock, Point, Rect, Size,
    };
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat};
    use windows::Media::Ocr::OcrEngine as WindowsOcrEngine;
    use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
    use windows::core::HSTRING;

    use crate::PlatformError;

    pub fn recognize_system_ocr(
        job: &OcrJob,
        image: &ImageData,
    ) -> Result<OcrResult, PlatformError> {
        let mut input = normalize_image(image)?;
        let max_dimension = winrt(
            "querying Windows OCR maximum image dimension",
            WindowsOcrEngine::MaxImageDimension(),
        )?;
        let scale = input.fit_within(max_dimension)?;
        let bitmap = software_bitmap_from_image(&input)?;
        let engine = create_engine(job.language_hint.as_deref())?;
        let language = engine
            .RecognizerLanguage()
            .ok()
            .and_then(|language| language.LanguageTag().ok())
            .map(|tag| tag.to_string());
        let result = winrt(
            "running Windows OCR",
            winrt("starting Windows OCR", engine.RecognizeAsync(&bitmap))?.get(),
        )?;

        let plain_text = winrt("reading Windows OCR text", result.Text())?.to_string();
        let lines = winrt("reading Windows OCR lines", result.Lines())?;
        let line_count = winrt("reading Windows OCR line count", lines.Size())?;
        let mut blocks = Vec::with_capacity(line_count as usize);

        for line_index in 0..line_count {
            let line = winrt("reading Windows OCR line", lines.GetAt(line_index))?;
            let text = winrt("reading Windows OCR line text", line.Text())?.to_string();
            if text.trim().is_empty() {
                continue;
            }

            let words = winrt("reading Windows OCR line words", line.Words())?;
            let word_count = winrt("reading Windows OCR word count", words.Size())?;
            let mut bounds = None;

            for word_index in 0..word_count {
                let word = winrt("reading Windows OCR word", words.GetAt(word_index))?;
                let word_bounds = scale.scale_rect(from_windows_rect(winrt(
                    "reading Windows OCR word bounds",
                    word.BoundingRect(),
                )?));
                bounds = Some(match bounds {
                    Some(current) => union_rect(current, word_bounds),
                    None => word_bounds,
                });
            }

            let bounds = bounds.unwrap_or_else(|| {
                Rect::new(
                    Point::ZERO,
                    Size::new(
                        image.metadata.pixel_size.width,
                        image.metadata.pixel_size.height,
                    ),
                )
            });
            blocks.push(OcrTextBlock {
                text,
                bounds,
                confidence: None,
                language: language.clone(),
            });
        }

        Ok(OcrResult {
            job_id: job.id.clone(),
            image_id: image.id.clone(),
            blocks,
            plain_text,
        })
    }

    struct NormalizedImage {
        width: u32,
        height: u32,
        rgba: RgbaImage,
    }

    impl NormalizedImage {
        fn fit_within(&mut self, max_dimension: u32) -> Result<CoordinateScale, PlatformError> {
            if max_dimension == 0 || (self.width <= max_dimension && self.height <= max_dimension) {
                return Ok(CoordinateScale::identity());
            }

            let ratio = (max_dimension as f32 / self.width.max(self.height) as f32).min(1.0);
            let resized_width = ((self.width as f32 * ratio).round() as u32).max(1);
            let resized_height = ((self.height as f32 * ratio).round() as u32).max(1);
            let resized = image::imageops::resize(
                &self.rgba,
                resized_width,
                resized_height,
                FilterType::Triangle,
            );
            let scale = CoordinateScale {
                x: self.width as f32 / resized_width as f32,
                y: self.height as f32 / resized_height as f32,
            };
            self.width = resized_width;
            self.height = resized_height;
            self.rgba = resized;
            Ok(scale)
        }
    }

    #[derive(Clone, Copy)]
    struct CoordinateScale {
        x: f32,
        y: f32,
    }

    impl CoordinateScale {
        fn identity() -> Self {
            Self { x: 1.0, y: 1.0 }
        }

        fn scale_rect(self, rect: Rect) -> Rect {
            Rect::new(
                Point::new(rect.origin.x * self.x, rect.origin.y * self.y),
                Size::new(rect.size.width * self.x, rect.size.height * self.y),
            )
        }
    }

    fn normalize_image(image: &ImageData) -> Result<NormalizedImage, PlatformError> {
        match image.metadata.format {
            ImageFormat::Png => {
                let decoded = image::load_from_memory(&image.bytes).map_err(|error| {
                    PlatformError::new(
                        "system_ocr_image_decode_failed",
                        format!("failed to decode PNG image for system OCR: {error}"),
                    )
                })?;
                let rgba = decoded.to_rgba8();
                let (width, height) = rgba.dimensions();
                Ok(NormalizedImage {
                    width,
                    height,
                    rgba,
                })
            }
            ImageFormat::Rgba8 | ImageFormat::Bgra8 => {
                let (width, height) = image_dimensions(image)?;
                let mut bytes = image.bytes.clone();
                let expected_len = width as usize * height as usize * 4;
                if bytes.len() != expected_len {
                    return Err(PlatformError::new(
                        "system_ocr_invalid_image",
                        format!(
                            "raw image byte length {} does not match {}x{} RGBA/BGRA data",
                            bytes.len(),
                            width,
                            height
                        ),
                    ));
                }

                if image.metadata.format == ImageFormat::Bgra8 {
                    for pixel in bytes.chunks_exact_mut(4) {
                        pixel.swap(0, 2);
                    }
                }

                let rgba = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
                    PlatformError::new(
                        "system_ocr_invalid_image",
                        "failed to build RGBA image for system OCR",
                    )
                })?;
                Ok(NormalizedImage {
                    width,
                    height,
                    rgba,
                })
            }
        }
    }

    fn image_dimensions(image: &ImageData) -> Result<(u32, u32), PlatformError> {
        let width = image.metadata.pixel_size.width.round();
        let height = image.metadata.pixel_size.height.round();
        if width <= 0.0 || height <= 0.0 || width > u32::MAX as f32 || height > u32::MAX as f32 {
            return Err(PlatformError::new(
                "system_ocr_invalid_image",
                "image dimensions are invalid for system OCR",
            ));
        }
        Ok((width as u32, height as u32))
    }

    fn software_bitmap_from_image(
        image: &NormalizedImage,
    ) -> Result<windows::Graphics::Imaging::SoftwareBitmap, PlatformError> {
        let png = encode_png(image)?;
        let stream = winrt(
            "creating Windows OCR input stream",
            InMemoryRandomAccessStream::new(),
        )?;
        let writer = winrt(
            "creating Windows OCR stream writer",
            DataWriter::CreateDataWriter(&stream),
        )?;
        winrt("writing Windows OCR image bytes", writer.WriteBytes(&png))?;
        winrt(
            "storing Windows OCR image bytes",
            winrt("starting Windows OCR stream store", writer.StoreAsync())?.get(),
        )?;
        winrt(
            "flushing Windows OCR image bytes",
            winrt("starting Windows OCR stream flush", writer.FlushAsync())?.get(),
        )?;
        winrt("detaching Windows OCR stream writer", writer.DetachStream())?;
        let _ = writer.Close();
        winrt("rewinding Windows OCR input stream", stream.Seek(0))?;

        let decoder = winrt(
            "decoding Windows OCR image",
            winrt(
                "starting Windows OCR image decoder",
                BitmapDecoder::CreateAsync(&stream),
            )?
            .get(),
        )?;
        winrt(
            "converting Windows OCR image",
            winrt(
                "starting Windows OCR image conversion",
                decoder.GetSoftwareBitmapConvertedAsync(
                    BitmapPixelFormat::Bgra8,
                    BitmapAlphaMode::Ignore,
                ),
            )?
            .get(),
        )
    }

    fn encode_png(image: &NormalizedImage) -> Result<Vec<u8>, PlatformError> {
        let mut png = Vec::new();
        let encoder = PngEncoder::new(&mut png);
        encoder
            .write_image(
                image.rgba.as_raw(),
                image.width,
                image.height,
                ColorType::Rgba8.into(),
            )
            .map_err(|error| {
                PlatformError::new(
                    "system_ocr_image_encode_failed",
                    format!("failed to encode image for system OCR: {error}"),
                )
            })?;
        Ok(png)
    }

    fn create_engine(language_hint: Option<&str>) -> Result<WindowsOcrEngine, PlatformError> {
        if let Some(tag) = language_hint.and_then(normalize_language_hint) {
            let tag = HSTRING::from(tag);
            let language = winrt(
                "creating Windows OCR language",
                Language::CreateLanguage(&tag),
            )?;
            let supported = winrt(
                "checking Windows OCR language support",
                WindowsOcrEngine::IsLanguageSupported(&language),
            )?;
            if supported {
                return winrt(
                    "creating Windows OCR engine for language",
                    WindowsOcrEngine::TryCreateFromLanguage(&language),
                );
            }
        }

        winrt(
            "creating Windows OCR engine from user profile languages",
            WindowsOcrEngine::TryCreateFromUserProfileLanguages(),
        )
    }

    fn normalize_language_hint(hint: &str) -> Option<String> {
        let hint = hint.trim();
        if hint.is_empty() || hint.eq_ignore_ascii_case("auto") {
            return None;
        }
        Some(match hint.to_ascii_lowercase().as_str() {
            "zh" | "cn" | "chs" | "zh_cn" => "zh-Hans".to_string(),
            "cht" | "zh_tw" | "zh-hant" => "zh-Hant".to_string(),
            "jp" | "ja" => "ja".to_string(),
            "kr" | "ko" => "ko".to_string(),
            "en" => "en".to_string(),
            _ => hint.replace('_', "-"),
        })
    }

    fn from_windows_rect(rect: windows::Foundation::Rect) -> Rect {
        Rect::new(
            Point::new(rect.X, rect.Y),
            Size::new(rect.Width, rect.Height),
        )
    }

    fn union_rect(a: Rect, b: Rect) -> Rect {
        let min_x = a.origin.x.min(b.origin.x);
        let min_y = a.origin.y.min(b.origin.y);
        let max_x = a.max_x().max(b.max_x());
        let max_y = a.max_y().max(b.max_y());
        Rect::new(
            Point::new(min_x, min_y),
            Size::new(max_x - min_x, max_y - min_y),
        )
    }

    fn winrt<T>(context: &str, result: windows::core::Result<T>) -> Result<T, PlatformError> {
        result.map_err(|error| {
            PlatformError::new(
                "system_ocr_winrt_failed",
                format!("{context} failed: {error}"),
            )
        })
    }
}
