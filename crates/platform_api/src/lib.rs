mod capabilities;
mod capture;
mod clipboard;
mod dialog;
mod error;
mod hotkey;
mod platform;
mod shared_memory;
mod window;

pub use capabilities::*;
pub use capture::*;
pub use clipboard::*;
pub use dialog::*;
pub use error::*;
pub use hotkey::*;
pub use platform::*;
pub use shared_memory::*;
pub use window::*;

pub use shared_models::{
    ImageData, ImageFormat, OcrJob, OcrResult, Point, Rect, ScaleFactor, Size,
};
