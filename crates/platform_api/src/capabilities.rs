#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    Supported,
    Degraded {
        reason: String,
    },
    NeedsSetup {
        reason: String,
        action: Option<String>,
    },
    PermissionDenied {
        reason: String,
    },
    Unavailable {
        reason: String,
    },
}

impl CapabilityStatus {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason: reason.into(),
        }
    }

    pub fn needs_setup(reason: impl Into<String>, action: Option<String>) -> Self {
        Self::NeedsSetup {
            reason: reason.into(),
            action,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub screen_capture: CapabilityStatus,
    pub overlay_window: CapabilityStatus,
    pub pin_window: CapabilityStatus,
    pub system_ocr: CapabilityStatus,
    pub clipboard_read: CapabilityStatus,
    pub clipboard_write: CapabilityStatus,
    pub global_hotkey: CapabilityStatus,
    pub file_dialog: CapabilityStatus,
    pub shared_memory: CapabilityStatus,
    pub secure_storage: CapabilityStatus,
}

impl PlatformCapabilities {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            screen_capture: CapabilityStatus::unavailable(reason.clone()),
            overlay_window: CapabilityStatus::unavailable(reason.clone()),
            pin_window: CapabilityStatus::unavailable(reason.clone()),
            system_ocr: CapabilityStatus::unavailable(reason.clone()),
            clipboard_read: CapabilityStatus::unavailable(reason.clone()),
            clipboard_write: CapabilityStatus::unavailable(reason.clone()),
            global_hotkey: CapabilityStatus::unavailable(reason.clone()),
            file_dialog: CapabilityStatus::unavailable(reason.clone()),
            shared_memory: CapabilityStatus::unavailable(reason.clone()),
            secure_storage: CapabilityStatus::unavailable(reason),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformCapability {
    ScreenCapture,
    OverlayWindow,
    PinWindow,
    SystemOcr,
    ClipboardRead,
    ClipboardWrite,
    GlobalHotkey,
    FileDialog,
    SharedMemory,
    SecureStorage,
}
