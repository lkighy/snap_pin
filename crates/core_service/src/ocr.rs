use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};

use shared_models::{CoreEvent, OcrJob};

pub struct OcrCoordinator {
    pending_jobs: Vec<OcrJob>,
    canceled_jobs: HashSet<String>,
    sender: Sender<CoreEvent>,
    receiver: Receiver<CoreEvent>,
}

impl Default for OcrCoordinator {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            pending_jobs: Vec::new(),
            canceled_jobs: HashSet::new(),
            sender,
            receiver,
        }
    }
}

impl OcrCoordinator {
    pub fn enqueue(&mut self, job: OcrJob) -> CoreEvent {
        let job_id = job.id.clone();
        self.pending_jobs.push(job);
        CoreEvent::OcrQueued { job_id }
    }

    pub fn pending_jobs(&self) -> &[OcrJob] {
        &self.pending_jobs
    }

    pub fn completion_sender(&self) -> Sender<CoreEvent> {
        self.sender.clone()
    }

    pub fn cancel(&mut self, job_id: String) -> CoreEvent {
        self.canceled_jobs.insert(job_id.clone());
        self.pending_jobs.retain(|job| job.id != job_id);
        CoreEvent::OcrCanceled { job_id }
    }

    pub fn is_canceled(&self, job_id: &str) -> bool {
        self.canceled_jobs.contains(job_id)
    }

    pub fn drain_completed(&mut self) -> Vec<CoreEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            if let CoreEvent::OcrCompleted { result } = &event {
                self.pending_jobs.retain(|job| job.id != result.job_id);
            }
            events.push(event);
        }
        events
    }
}
