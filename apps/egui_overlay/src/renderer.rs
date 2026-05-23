#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceKind {
    TransparentOverlay,
    PinWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererPlan {
    pub surface: RenderSurfaceKind,
    pub uses_gpu: bool,
    pub supports_text_overlays: bool,
}

impl RendererPlan {
    pub fn overlay() -> Self {
        Self {
            surface: RenderSurfaceKind::TransparentOverlay,
            uses_gpu: true,
            supports_text_overlays: true,
        }
    }
}
