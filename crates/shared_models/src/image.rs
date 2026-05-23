use crate::{Rect, Size};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageId(pub String);

impl ImageId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Bgra8,
    Rgba8,
    Png,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageMetadata {
    pub id: ImageId,
    pub pixel_size: Size,
    pub format: ImageFormat,
    pub monitor_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageData {
    pub id: ImageId,
    pub metadata: ImageMetadata,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextOverlayRole {
    Ocr,
    Translation,
    UserNote,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextOverlay {
    pub text: String,
    pub language: Option<String>,
    pub bounds: Rect,
    pub role: TextOverlayRole,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PinnedImage {
    pub id: ImageId,
    pub bounds: Rect,
    pub opacity: f32,
    pub click_through: bool,
    pub always_on_top: bool,
    pub text_overlays: Vec<TextOverlay>,
}

impl PinnedImage {
    pub fn new(id: ImageId, bounds: Rect) -> Self {
        Self {
            id,
            bounds,
            opacity: 1.0,
            click_through: false,
            always_on_top: true,
            text_overlays: Vec::new(),
        }
    }
}
