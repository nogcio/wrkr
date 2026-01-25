use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct IterationGate {
    counter: AtomicU64,
    iterations: Option<u64>,
    duration: Option<Duration>,
    deadline: OnceLock<Instant>,
}

impl IterationGate {
    pub fn new(iterations: Option<u64>, duration: Option<Duration>) -> Self {
        Self {
            counter: AtomicU64::new(0),
            iterations,
            duration,
            deadline: OnceLock::new(),
        }
    }

    pub fn start_at(&self, started: Instant) {
        if self.deadline.get().is_some() {
            return;
        }

        if let Some(duration) = self.duration {
            let _ = self.deadline.set(started + duration);
        }
    }

    pub fn start(&self) {
        self.start_at(Instant::now());
    }

    pub fn next(&self) -> bool {
        // Hot path: avoid timekeeping entirely unless we're in duration mode.
        if self.duration.is_some() {
            let now = Instant::now();

            // If the runner didn't explicitly set a start time, lazily initialize the deadline
            // from the first observed iteration.
            if self.deadline.get().is_none() {
                self.start_at(now);
            }

            if let Some(deadline) = self.deadline.get()
                && now >= *deadline
            {
                return false;
            }
        }

        if let Some(total) = self.iterations {
            let idx = self.counter.fetch_add(1, Ordering::Relaxed);
            if idx >= total {
                return false;
            }
        } else if self.duration.is_none() {
            // Neither iterations nor duration => run once.
            let idx = self.counter.fetch_add(1, Ordering::Relaxed);
            if idx > 0 {
                return false;
            }
        }

        true
    }
}
