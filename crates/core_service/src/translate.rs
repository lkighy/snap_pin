use shared_models::{CoreEvent, TranslationRequest};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TranslateCoordinator {
    pending_requests: Vec<TranslationRequest>,
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
}
