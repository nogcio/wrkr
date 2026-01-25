use std::time::Duration;

use super::config::Stage;

#[derive(Debug, Clone)]
pub struct StageSnapshot {
    pub index: usize,
    pub count: usize,
    pub stage_elapsed: Duration,
    pub stage_remaining: Duration,
    pub start_target: u64,
    pub end_target: u64,
    pub current_target: u64,
}

#[derive(Debug, Clone)]
pub struct RampingU64Schedule {
    start: u64,
    stages: Vec<Stage>,
    cumulative_ends: Vec<Duration>,
}

impl RampingU64Schedule {
    pub fn new(start: u64, stages: Vec<Stage>) -> Self {
        let mut cumulative_ends = Vec::with_capacity(stages.len());
        let mut acc = Duration::ZERO;
        for s in &stages {
            acc = acc.saturating_add(s.duration);
            cumulative_ends.push(acc);
        }

        Self {
            start,
            stages,
            cumulative_ends,
        }
    }

    pub fn stages(&self) -> &[Stage] {
        &self.stages
    }

    pub fn total_duration(&self) -> Duration {
        self.cumulative_ends
            .last()
            .copied()
            .unwrap_or(Duration::ZERO)
    }

    pub fn is_done(&self, elapsed: Duration) -> bool {
        elapsed >= self.total_duration()
    }

    pub fn target_at(&self, elapsed: Duration) -> u64 {
        if self.stages.is_empty() {
            return self.start;
        }

        if elapsed == Duration::ZERO {
            return self.start;
        }

        let total = self.total_duration();
        if elapsed >= total {
            return self.stages.last().map(|s| s.target).unwrap_or(self.start);
        }

        let idx = match self
            .cumulative_ends
            .binary_search_by(|end| end.cmp(&elapsed))
        {
            Ok(i) => i,
            Err(i) => i,
        };

        let stage_end = self.cumulative_ends[idx];
        let stage_start = if idx == 0 {
            Duration::ZERO
        } else {
            self.cumulative_ends[idx - 1]
        };

        let stage = &self.stages[idx];
        let stage_duration = stage_end.saturating_sub(stage_start);
        let stage_elapsed = elapsed.saturating_sub(stage_start);

        let start_target = if idx == 0 {
            self.start
        } else {
            self.stages[idx - 1].target
        };
        let end_target = stage.target;

        if stage_duration.is_zero() {
            return end_target;
        }

        // Linear interpolation across the stage.
        let start_i = start_target as i128;
        let end_i = end_target as i128;
        let delta = end_i - start_i;

        let num = stage_elapsed.as_nanos() as i128;
        let den = stage_duration.as_nanos() as i128;

        let cur = start_i + (delta.saturating_mul(num) / den.max(1));
        cur.clamp(0, u64::MAX as i128) as u64
    }

    pub fn stage_snapshot_at(&self, elapsed: Duration) -> Option<StageSnapshot> {
        if self.stages.is_empty() {
            return None;
        }

        let total = self.total_duration();
        let clamped = elapsed.min(total);

        let idx = if clamped >= total {
            self.stages.len().saturating_sub(1)
        } else {
            match self
                .cumulative_ends
                .binary_search_by(|end| end.cmp(&clamped))
            {
                Ok(i) => i,
                Err(i) => i,
            }
        };

        let stage_end = self.cumulative_ends[idx];
        let stage_start = if idx == 0 {
            Duration::ZERO
        } else {
            self.cumulative_ends[idx - 1]
        };

        let stage_duration = stage_end.saturating_sub(stage_start);
        let stage_elapsed = clamped.saturating_sub(stage_start);
        let stage_remaining = stage_duration.saturating_sub(stage_elapsed);

        let start_target = if idx == 0 {
            self.start
        } else {
            self.stages[idx - 1].target
        };
        let end_target = self.stages[idx].target;

        Some(StageSnapshot {
            index: idx,
            count: self.stages.len(),
            stage_elapsed,
            stage_remaining,
            start_target,
            end_target,
            current_target: self.target_at(clamped),
        })
    }

    pub fn next_recheck_in(&self, elapsed: Duration, vu_index: u64) -> Duration {
        // Conservative default.
        let default_sleep = Duration::from_millis(50);

        if self.stages.is_empty() {
            return default_sleep;
        }

        let total = self.total_duration();
        if elapsed >= total {
            return Duration::ZERO;
        }

        let idx = match self
            .cumulative_ends
            .binary_search_by(|end| end.cmp(&elapsed))
        {
            Ok(i) => i,
            Err(i) => i,
        };

        let stage_end = self.cumulative_ends[idx];
        let stage_start = if idx == 0 {
            Duration::ZERO
        } else {
            self.cumulative_ends[idx - 1]
        };

        let stage = &self.stages[idx];
        let stage_duration = stage_end.saturating_sub(stage_start);
        let stage_elapsed = elapsed.saturating_sub(stage_start);

        let start_target = if idx == 0 {
            self.start
        } else {
            self.stages[idx - 1].target
        };
        let end_target = stage.target;

        // If we're already active, a short sleep is fine to pick up ramp-down promptly.
        let cur_target = self.target_at(elapsed);
        if vu_index <= cur_target {
            return Duration::from_millis(1);
        }

        // If target is decreasing, this VU can't become active within this stage.
        if end_target <= start_target {
            return stage_end.saturating_sub(elapsed).min(default_sleep);
        }

        // Target is increasing: compute when the ramp reaches this VU index.
        // Solve for t where start + (end-start)*t/dur >= vu_index.
        let start_i = start_target as i128;
        let end_i = end_target as i128;
        let want = vu_index as i128;

        let delta = end_i - start_i;
        if delta <= 0 {
            return default_sleep;
        }

        if want <= start_i {
            return Duration::from_millis(0);
        }
        if want > end_i {
            return stage_end.saturating_sub(elapsed).min(default_sleep);
        }

        let stage_ns = stage_duration.as_nanos() as i128;
        let elapsed_ns = stage_elapsed.as_nanos() as i128;

        let needed_ns = ((want - start_i).saturating_mul(stage_ns) / delta).max(0);
        let wait_ns = needed_ns.saturating_sub(elapsed_ns).max(0);
        let wait = Duration::from_nanos(wait_ns.min(u64::MAX as i128) as u64);

        wait.min(default_sleep)
    }
}
