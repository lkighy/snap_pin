use shared_models::{ImageId, PinnedImage, Rect};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PinManager {
    pins: Vec<PinnedImage>,
}

impl PinManager {
    pub fn add_pin(&mut self, image_id: ImageId, bounds: Rect) {
        self.pins.push(PinnedImage::new(image_id, bounds));
    }

    pub fn pins(&self) -> &[PinnedImage] {
        &self.pins
    }
}
