use std::path::PathBuf;

use crate::PlatformError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPayload {
    Text(String),
    ImageRgba {
        width: usize,
        height: usize,
        bytes: Vec<u8>,
    },
    Files(Vec<PathBuf>),
}

pub trait Clipboard: Send + Sync {
    fn read(&self) -> Result<ClipboardPayload, PlatformError>;
    fn write(&self, payload: ClipboardPayload) -> Result<(), PlatformError>;
}
