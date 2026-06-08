use std::fmt::Display;
use std::time::{Duration, Instant};

#[must_use]
pub struct PerfSpan {
    name: &'static str,
    start: Instant,
    fields: Vec<PerfField>,
    finished: bool,
}

struct PerfField {
    key: &'static str,
    value: String,
}

impl PerfSpan {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
            fields: Vec::new(),
            finished: false,
        }
    }

    pub fn field(mut self, key: &'static str, value: impl Display) -> Self {
        self.fields.push(PerfField {
            key,
            value: value.to_string(),
        });
        self
    }

    pub fn add_field(&mut self, key: &'static str, value: impl Display) {
        self.fields.push(PerfField {
            key,
            value: value.to_string(),
        });
    }

    pub fn finish(mut self) -> Duration {
        self.finished = true;
        let duration = self.start.elapsed();
        log_perf(self.name, duration, &self.fields);
        duration
    }
}

impl Drop for PerfSpan {
    fn drop(&mut self) {
        if !self.finished {
            log_perf(self.name, self.start.elapsed(), &self.fields);
        }
    }
}

pub fn log_elapsed(name: &'static str, start: Instant) -> Duration {
    let duration = start.elapsed();
    log_perf(name, duration, &[]);
    duration
}

pub fn log_elapsed_with(
    name: &'static str,
    start: Instant,
    fields: &[(&'static str, String)],
) -> Duration {
    let duration = start.elapsed();
    let fields = fields
        .iter()
        .map(|(key, value)| PerfField {
            key,
            value: value.clone(),
        })
        .collect::<Vec<_>>();
    log_perf(name, duration, &fields);
    duration
}

fn log_perf(name: &'static str, duration: Duration, fields: &[PerfField]) {
    let duration_ms = duration.as_micros() as f64 / 1000.0;
    if fields.is_empty() {
        log::info!(target: "perf", "perf span={name} duration_ms={duration_ms:.3}");
        return;
    }

    let fields = fields
        .iter()
        .map(|field| format!("{}={}", field.key, sanitize_field_value(&field.value)))
        .collect::<Vec<_>>()
        .join(" ");
    log::info!(
        target: "perf",
        "perf span={name} duration_ms={duration_ms:.3} {fields}"
    );
}

fn sanitize_field_value(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
