use std::time::Duration;

use eframe::egui::{Context, Pos2, Vec2, ViewportCommand, WindowLevel};
use platform_api::NativeWindowRef;
use raw_window_handle::RawWindowHandle;
use shared_models::{Point, Rect, Size};

use crate::runtime::control::SharedSnapshotCommand;

const RESIDENT_IDLE_SIZE: f32 = 1.0;
const RESIDENT_IDLE_X: f32 = -32000.0;
const RESIDENT_IDLE_Y: f32 = -32000.0;
const RESIDENT_IDLE_REPAINT_MS: u64 = 100;

pub(crate) fn park_resident_window(ctx: &Context, hwnd: Option<isize>) {
    // Keep the resident window alive but invisible to the user. Fully hidden
    // windows may stop receiving repaint wakeups, leaving future commands queued.
    if let Some(hwnd) = hwnd {
        if let Err(error) = platform_runtime::create_platform()
            .window_ops()
            .park_window(
                NativeWindowRef::from_raw(hwnd),
                Rect::new(
                    Point::new(RESIDENT_IDLE_X, RESIDENT_IDLE_Y),
                    Size::new(RESIDENT_IDLE_SIZE, RESIDENT_IDLE_SIZE),
                ),
            )
        {
            log::warn!("failed to park resident capture window: {error}");
        }
    }
    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(
        RESIDENT_IDLE_X,
        RESIDENT_IDLE_Y,
    )));
    ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
        RESIDENT_IDLE_SIZE,
        RESIDENT_IDLE_SIZE,
    )));
    ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
}

pub(crate) fn request_resident_idle_repaint(ctx: &Context) {
    ctx.request_repaint_after(Duration::from_millis(RESIDENT_IDLE_REPAINT_MS));
}

pub(crate) fn show_capture_window(ctx: &Context, snapshot: &SharedSnapshotCommand) {
    ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(
        snapshot.origin_x,
        snapshot.origin_y,
    )));
    ctx.send_viewport_cmd(ViewportCommand::InnerSize(Vec2::new(
        snapshot.width as f32,
        snapshot.height as f32,
    )));
    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(ViewportCommand::Focus);
    ctx.request_repaint();
}

pub(crate) fn hwnd_from_raw_window_handle(handle: RawWindowHandle) -> Option<isize> {
    match handle {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}
