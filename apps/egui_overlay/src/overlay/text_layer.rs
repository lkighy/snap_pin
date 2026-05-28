use shared_models::{Rect, TextOverlay, TextOverlayRole};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TextLayer {
    overlays: Vec<TextOverlay>,
}

impl TextLayer {
    pub fn set_ocr_text(&mut self, text: impl Into<String>, bounds: Rect) {
        self.overlays.push(TextOverlay {
            text: text.into(),
            language: None,
            bounds,
            role: TextOverlayRole::Ocr,
            confidence: None,
        });
    }

    pub fn set_translation_text(
        &mut self,
        text: impl Into<String>,
        language: impl Into<String>,
        bounds: Rect,
    ) {
        self.overlays.push(TextOverlay {
            text: text.into(),
            language: Some(language.into()),
            bounds,
            role: TextOverlayRole::Translation,
            confidence: None,
        });
    }

    pub fn overlays(&self) -> &[TextOverlay] {
        &self.overlays
    }
}
