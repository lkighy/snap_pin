use shared_models::{CoreEvent, OcrJob};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct OcrCoordinator {
    pending_jobs: Vec<OcrJob>,
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
}
