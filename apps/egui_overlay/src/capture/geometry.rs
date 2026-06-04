use eframe::egui::{Pos2, Rect as EguiRect, Vec2};

const SELECTION_EDGE_HIT_SIZE: f32 = 8.0;
const SELECTION_MIN_SIZE: f32 = 12.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CaptureDragState {
    pub(crate) start: Pos2,
    pub(crate) original: EguiRect,
    pub(crate) mode: CaptureDragMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureDragMode {
    Create,
    Move,
    Resize(ResizeEdges),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResizeEdges {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

pub(crate) fn clamp_pos(pos: Pos2, rect: EguiRect) -> Pos2 {
    Pos2::new(
        pos.x.clamp(rect.min.x, rect.max.x),
        pos.y.clamp(rect.min.y, rect.max.y),
    )
}

pub(crate) fn selection_drag_mode(selection: EguiRect, position: Pos2) -> CaptureDragMode {
    let near_left = (position.x - selection.min.x).abs() <= SELECTION_EDGE_HIT_SIZE;
    let near_right = (position.x - selection.max.x).abs() <= SELECTION_EDGE_HIT_SIZE;
    let near_top = (position.y - selection.min.y).abs() <= SELECTION_EDGE_HIT_SIZE;
    let near_bottom = (position.y - selection.max.y).abs() <= SELECTION_EDGE_HIT_SIZE;
    let edges = ResizeEdges {
        left: near_left,
        right: near_right,
        top: near_top,
        bottom: near_bottom,
    };

    if edges.left || edges.right || edges.top || edges.bottom {
        CaptureDragMode::Resize(edges)
    } else if selection.contains(position) {
        CaptureDragMode::Move
    } else {
        CaptureDragMode::Create
    }
}

pub(crate) fn apply_drag(drag: CaptureDragState, position: Pos2, canvas: EguiRect) -> EguiRect {
    match drag.mode {
        CaptureDragMode::Create => normalize_selection(EguiRect::from_two_pos(
            drag.start,
            clamp_pos(position, canvas),
        )),
        CaptureDragMode::Move => {
            let delta = position - drag.start;
            let mut rect = drag.original.translate(delta);
            if rect.min.x < canvas.min.x {
                rect = rect.translate(Vec2::new(canvas.min.x - rect.min.x, 0.0));
            }
            if rect.min.y < canvas.min.y {
                rect = rect.translate(Vec2::new(0.0, canvas.min.y - rect.min.y));
            }
            if rect.max.x > canvas.max.x {
                rect = rect.translate(Vec2::new(canvas.max.x - rect.max.x, 0.0));
            }
            if rect.max.y > canvas.max.y {
                rect = rect.translate(Vec2::new(0.0, canvas.max.y - rect.max.y));
            }
            rect
        }
        CaptureDragMode::Resize(edges) => resize_selection(drag.original, position, canvas, edges),
    }
}

fn resize_selection(
    selection: EguiRect,
    position: Pos2,
    canvas: EguiRect,
    edges: ResizeEdges,
) -> EguiRect {
    let position = clamp_pos(position, canvas);
    let mut min = selection.min;
    let mut max = selection.max;

    if edges.left {
        min.x = position.x.min(max.x - SELECTION_MIN_SIZE);
    }
    if edges.right {
        max.x = position.x.max(min.x + SELECTION_MIN_SIZE);
    }
    if edges.top {
        min.y = position.y.min(max.y - SELECTION_MIN_SIZE);
    }
    if edges.bottom {
        max.y = position.y.max(min.y + SELECTION_MIN_SIZE);
    }

    normalize_selection(EguiRect::from_min_max(
        clamp_pos(min, canvas),
        clamp_pos(max, canvas),
    ))
}

pub(crate) fn normalize_selection(selection: EguiRect) -> EguiRect {
    EguiRect::from_min_max(
        Pos2::new(
            selection.min.x.min(selection.max.x),
            selection.min.y.min(selection.max.y),
        ),
        Pos2::new(
            selection.min.x.max(selection.max.x),
            selection.min.y.max(selection.max.y),
        ),
    )
}
