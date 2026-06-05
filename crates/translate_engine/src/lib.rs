mod ct2_backend;
mod engine;
mod error;
mod local;
mod mock;
mod model_bundle;
mod router;

pub use engine::*;
pub use error::*;
pub use local::*;
pub use mock::*;
pub use model_bundle::*;
pub use router::*;

pub fn local_runtime_status() -> &'static str {
    ct2_backend::runtime_status()
}
