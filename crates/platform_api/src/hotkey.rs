use crate::PlatformError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyRegistration {
    pub id: String,
    pub accelerator: String,
}

impl HotkeyRegistration {
    pub fn new(id: impl Into<String>, accelerator: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            accelerator: accelerator.into(),
        }
    }
}

pub type HotkeyEventSink = Box<dyn Fn(HotkeyRegistration) + Send + 'static>;

pub trait HotkeyToken: Send {}

pub trait GlobalHotkey: Send + Sync {
    fn register(
        &self,
        registration: HotkeyRegistration,
        sink: HotkeyEventSink,
    ) -> Result<Box<dyn HotkeyToken>, PlatformError>;
}
