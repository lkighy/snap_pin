use ipc::{InMemoryBus, IpcBus, IpcEnvelope, IpcError};

pub struct DesktopIpcBridge {
    bus: InMemoryBus,
}

impl Default for DesktopIpcBridge {
    fn default() -> Self {
        Self {
            bus: InMemoryBus::default(),
        }
    }
}

impl DesktopIpcBridge {
    pub fn send(&mut self, envelope: IpcEnvelope) -> Result<(), IpcError> {
        self.bus.send(envelope)
    }

    pub fn drain(&mut self) -> Vec<IpcEnvelope> {
        let mut envelopes = Vec::new();
        while let Some(envelope) = self.bus.try_recv() {
            envelopes.push(envelope);
        }
        envelopes
    }
}
