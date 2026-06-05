use std::sync::mpsc::{self, Receiver, Sender};

use shared_models::{CoreEvent, TranslationRequest};

pub struct TranslateCoordinator {
    pending_requests: Vec<TranslationRequest>,
    sender: Sender<CoreEvent>,
    receiver: Receiver<CoreEvent>,
}

impl Default for TranslateCoordinator {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            pending_requests: Vec::new(),
            sender,
            receiver,
        }
    }
}

impl TranslateCoordinator {
    pub fn enqueue(&mut self, request: TranslationRequest) -> CoreEvent {
        let request_id = request.id.clone();
        self.pending_requests.push(request);
        CoreEvent::TranslationQueued { request_id }
    }

    pub fn pending_requests(&self) -> &[TranslationRequest] {
        &self.pending_requests
    }

    pub fn completion_sender(&self) -> Sender<CoreEvent> {
        self.sender.clone()
    }

    pub fn drain_completed(&mut self) -> Vec<CoreEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            if let CoreEvent::TranslationCompleted { result } = &event {
                self.pending_requests
                    .retain(|request| request.id != result.request_id);
            }
            events.push(event);
        }
        events
    }
}
