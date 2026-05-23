use shared_models::{CoreEvent, ImageData, ImageId, Rect};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ScreenshotCoordinator {
    active: bool,
    images: Vec<ImageData>,
}

impl ScreenshotCoordinator {
    pub fn start_capture(&mut self) -> CoreEvent {
        self.active = true;
        CoreEvent::CaptureStarted
    }

    pub fn cancel_capture(&mut self) -> CoreEvent {
        self.active = false;
        CoreEvent::CaptureCanceled
    }

    pub fn store_image(&mut self, image: ImageData) {
        self.active = false;
        if let Some(existing) = self.images.iter_mut().find(|item| item.id == image.id) {
            *existing = image;
        } else {
            self.images.push(image);
        }
    }

    pub fn image(&self, image_id: &ImageId) -> Option<&ImageData> {
        self.images.iter().find(|image| &image.id == image_id)
    }

    pub fn pin_image(&self, image_id: ImageId, _bounds: Rect) -> CoreEvent {
        CoreEvent::ImagePinned { image_id }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}
