#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardManager {
    available: bool,
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self { available: true }
    }
}

impl ClipboardManager {
    pub fn is_available(&self) -> bool {
        self.available
    }
}
