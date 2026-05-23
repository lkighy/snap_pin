#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyManager {
    enabled: bool,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl HotkeyManager {
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}
