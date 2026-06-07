use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::PlatformCapability;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    pub code: String,
    pub message: String,
    pub capability: Option<PlatformCapability>,
    pub recoverable: bool,
}

impl PlatformError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            capability: None,
            recoverable: false,
        }
    }

    pub fn recoverable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            capability: None,
            recoverable: true,
        }
    }

    pub fn with_capability(mut self, capability: PlatformCapability) -> Self {
        self.capability = Some(capability);
        self
    }

    pub fn with_recoverable(mut self, recoverable: bool) -> Self {
        self.recoverable = recoverable;
        self
    }
}

impl Display for PlatformError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for PlatformError {}
