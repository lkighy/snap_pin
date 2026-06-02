mod engine;
mod error;
mod external_api;
mod image_input;
mod local;
mod mock;
mod model_bundle;
mod ocr_rs_backend;
mod router;

pub use engine::*;
pub use error::*;
pub use external_api::*;
pub use image_input::*;
pub use local::*;
pub use mock::*;
pub use model_bundle::*;
pub use router::*;

pub fn local_runtime_status() -> &'static str {
    ocr_rs_backend::runtime_status()
}
