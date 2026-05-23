use shared_models::{Point, Rect};

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationTool {
    Arrow,
    Rectangle,
    Pen,
    Mosaic,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub tool: AnnotationTool,
    pub bounds: Rect,
    pub points: Vec<Point>,
}
