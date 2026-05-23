use std::collections::VecDeque;
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::IpcEnvelope;

pub trait IpcBus {
    fn send(&mut self, envelope: IpcEnvelope) -> Result<(), IpcError>;
    fn try_recv(&mut self) -> Option<IpcEnvelope>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcError {
    pub message: String,
}

impl IpcError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for IpcError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for IpcError {}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct InMemoryBus {
    queue: VecDeque<IpcEnvelope>,
}

impl IpcBus for InMemoryBus {
    fn send(&mut self, envelope: IpcEnvelope) -> Result<(), IpcError> {
        self.queue.push_back(envelope);
        Ok(())
    }

    fn try_recv(&mut self) -> Option<IpcEnvelope> {
        self.queue.pop_front()
    }
}
