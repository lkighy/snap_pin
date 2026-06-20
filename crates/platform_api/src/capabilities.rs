#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    Supported,
    Degraded {
        reason_code: String,
        reason: String,
    },
    NeedsSetup {
        reason_code: String,
        reason: String,
        action: Option<String>,
    },
    PermissionDenied {
        reason_code: String,
        reason: String,
    },
    Unavailable {
        reason_code: String,
        reason: String,
    },
}

impl CapabilityStatus {
    pub fn unavailable(reason_code: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason_code: reason_code.into(),
            reason: reason.into(),
        }
    }

    pub fn needs_setup(
        reason_code: impl Into<String>,
        reason: impl Into<String>,
        action: Option<String>,
    ) -> Self {
        Self::NeedsSetup {
            reason_code: reason_code.into(),
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
    pub fn unavailable(reason_code: impl Into<String>, reason: impl Into<String>) -> Self {
        let reason_code = reason_code.into();
        let reason = reason.into();
        Self {
            screen_capture: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            overlay_window: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            pin_window: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            system_ocr: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            clipboard_read: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            clipboard_write: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            global_hotkey: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            file_dialog: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            shared_memory: CapabilityStatus::unavailable(reason_code.clone(), reason.clone()),
            secure_storage: CapabilityStatus::unavailable(reason_code, reason),
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
