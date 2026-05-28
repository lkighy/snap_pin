use shared_models::{Point, Rect};

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayInput {
    PointerDown(Point),
    PointerMove(Point),
    PointerUp(Point),
    Escape,
    Confirm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionDrag {
    anchor: Point,
    current: Point,
}

impl SelectionDrag {
    pub fn new(anchor: Point) -> Self {
        Self {
            anchor,
            current: anchor,
        }
    }

    pub fn update(&mut self, current: Point) {
        self.current = current;
    }

    pub fn rect(&self) -> Rect {
        Rect::from_points(self.anchor, self.current)
    }
}
