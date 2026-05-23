#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn from_points(a: Point, b: Point) -> Self {
        let min_x = a.x.min(b.x);
        let min_y = a.y.min(b.y);
        let max_x = a.x.max(b.x);
        let max_y = a.y.max(b.y);

        Self {
            origin: Point::new(min_x, min_y),
            size: Size::new(max_x - min_x, max_y - min_y),
        }
    }

    pub fn max_x(self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn max_y(self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn area(self) -> f32 {
        self.size.width.max(0.0) * self.size.height.max(0.0)
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.x <= self.max_x()
            && point.y >= self.origin.y
            && point.y <= self.max_y()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFactor(pub f32);

impl Default for ScaleFactor {
    fn default() -> Self {
        Self(1.0)
    }
}
