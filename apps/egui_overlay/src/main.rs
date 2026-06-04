#![allow(dead_code)]

mod capture;
mod overlay;
mod pin;
mod runtime;

use eframe::NativeOptions;
use eframe::egui::{Vec2, ViewportBuilder, WindowLevel};

use capture::app::CaptureOverlayApp;
use pin::app::PinWindowApp;
use pin::window::{PinWindowSizing, clamp_pin_window_size};
use runtime::cli::{CliArgs, OverlayRunMode};
use runtime::logging::init_logging;
use runtime::text::OverlayText;

const RESIDENT_IDLE_SIZE: f32 = 1.0;
const RESIDENT_IDLE_X: f32 = -32000.0;
const RESIDENT_IDLE_Y: f32 = -32000.0;

fn main() -> eframe::Result<()> {
    init_logging();
    let args = CliArgs::parse();
    log::info!(
        "overlay starting mode={:?} resident={} snapshot={:?} image={:?}",
        args.mode,
        args.resident,
        args.snapshot,
        args.image
    );
    if matches!(args.mode, OverlayRunMode::Capture) && !args.resident && args.snapshot.is_none() {
        log::error!("capture overlay missing --snapshot in non-resident mode");
        eprintln!("snap pin capture overlay requires --snapshot <path> or --resident");
        return Ok(());
    }

    let options = native_options(&args);
    let title_text = OverlayText::new(args.language);
    let title = match args.mode {
        OverlayRunMode::Capture => title_text.capture_title,
        OverlayRunMode::Pin => "snap pin",
    };

    eframe::run_native(
        title,
        options,
        Box::new(move |creation_context| match args.mode {
            OverlayRunMode::Capture => Ok(Box::new(CaptureOverlayApp::new(creation_context, args))),
            OverlayRunMode::Pin => Ok(Box::new(PinWindowApp::new(creation_context, args))),
        }),
    )
}

fn native_options(args: &CliArgs) -> NativeOptions {
    let viewport = match args.mode {
        OverlayRunMode::Capture => {
            let viewport = ViewportBuilder::default()
                .with_decorations(false)
                .with_transparent(false)
                .with_always_on_top()
                .with_resizable(false)
                .with_taskbar(false);

            if args.resident {
                viewport
                    .with_position([RESIDENT_IDLE_X, RESIDENT_IDLE_Y])
                    .with_inner_size([RESIDENT_IDLE_SIZE, RESIDENT_IDLE_SIZE])
                    .with_transparent(true)
                    .with_visible(true)
            } else {
                viewport
                    .with_position([args.x, args.y])
                    .with_inner_size([args.width, args.height])
                    .with_visible(true)
            }
        }
        OverlayRunMode::Pin => {
            let level = if args.pin_always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            };
            let sizing = PinWindowSizing::new(args.pin_min_width, args.pin_min_height);
            let pin_size = clamp_pin_window_size(Vec2::new(args.width, args.height), sizing);
            ViewportBuilder::default()
                .with_decorations(false)
                .with_transparent(true)
                .with_window_level(level)
                .with_resizable(true)
                .with_taskbar(false)
                .with_position([args.x, args.y])
                .with_inner_size([pin_size.x, pin_size.y])
                .with_min_inner_size([sizing.min_width, sizing.min_height])
        }
    };

    NativeOptions {
        viewport,
        ..Default::default()
    }
}
