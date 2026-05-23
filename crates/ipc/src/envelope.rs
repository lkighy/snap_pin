use shared_models::{CoreCommand, CoreEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcSource {
    TauriUi,
    Core,
    Overlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcTarget {
    TauriUi,
    Core,
    Overlay,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IpcPayload {
    Command(CoreCommand),
    Event(CoreEvent),
    HealthCheck,
    HealthAck,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IpcEnvelope {
    pub id: String,
    pub source: IpcSource,
    pub target: IpcTarget,
    pub payload: IpcPayload,
}

impl IpcEnvelope {
    pub fn new(
        id: impl Into<String>,
        source: IpcSource,
        target: IpcTarget,
        payload: IpcPayload,
    ) -> Self {
        Self {
            id: id.into(),
            source,
            target,
            payload,
        }
    }
}
