use shared_models::{CoreCommand, CoreEvent, PinnedImage, Rect};

use crate::annotation::Annotation;
use crate::input::{OverlayInput, SelectionDrag};
use crate::text_layer::TextLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMode {
    Idle,
    Selecting,
    Annotating,
    Pinning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayApp {
    pub mode: OverlayMode,
    pub selection: Option<Rect>,
    pub pins: Vec<PinnedImage>,
    pub annotations: Vec<Annotation>,
    pub text_layer: TextLayer,
    drag: Option<SelectionDrag>,
}

impl Default for OverlayApp {
    fn default() -> Self {
        Self {
            mode: OverlayMode::Idle,
            selection: None,
            pins: Vec::new(),
            annotations: Vec::new(),
            text_layer: TextLayer::default(),
            drag: None,
        }
    }
}

impl OverlayApp {
    pub fn boot_summary(&self) -> String {
        "snap_pin overlay shell ready: egui/wgpu integration point".to_owned()
    }

    pub fn handle_input(&mut self, input: OverlayInput) -> Option<CoreCommand> {
        match input {
            OverlayInput::PointerDown(point) => {
                self.mode = OverlayMode::Selecting;
                self.drag = Some(SelectionDrag::new(point));
                None
            }
            OverlayInput::PointerMove(point) => {
                if let Some(drag) = &mut self.drag {
                    drag.update(point);
                    self.selection = Some(drag.rect());
                }
                None
            }
            OverlayInput::PointerUp(point) => {
                if let Some(drag) = &mut self.drag {
                    drag.update(point);
                    self.selection = Some(drag.rect());
                }
                self.drag = None;
                self.mode = OverlayMode::Idle;
                None
            }
            OverlayInput::Escape => {
                self.drag = None;
                self.selection = None;
                self.mode = OverlayMode::Idle;
                Some(CoreCommand::CancelCapture)
            }
            OverlayInput::Confirm => Some(CoreCommand::StartCapture),
        }
    }

    pub fn apply_core_event(&mut self, event: &CoreEvent) {
        match event {
            CoreEvent::OcrCompleted { result } => {
                for block in &result.blocks {
                    self.text_layer.set_ocr_text(&block.text, block.bounds);
                }
            }
            CoreEvent::TranslationCompleted { result } => {
                let bounds = self.selection.unwrap_or_default();
                self.text_layer.set_translation_text(
                    &result.translated_text,
                    &result.target_language.0,
                    bounds,
                );
            }
            _ => {}
        }
    }
}
