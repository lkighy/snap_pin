use std::path::PathBuf;

use crate::PlatformError;

pub trait FileDialog: Send + Sync {
    fn pick_folder(&self, title: &str) -> Result<Option<PathBuf>, PlatformError>;
    fn save_png_path(&self, default_name: &str) -> Result<Option<PathBuf>, PlatformError>;
}
