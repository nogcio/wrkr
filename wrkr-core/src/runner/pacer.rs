use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tokio::sync::Notify;

#[derive(Debug)]
pub struct ArrivalPacer {
    scheduled_total: AtomicU64,
    claimed_total: AtomicU64,
    dropped_total: AtomicU64,

    active_vus: AtomicU64,
    pre_allocated_vus: u64,
    max_vus: u64,

    done: AtomicBool,
    notify: Notify,
}

impl ArrivalPacer {
    pub fn new(pre_allocated_vus: u64, max_vus: u64) -> Self {
        Self {
            scheduled_total: AtomicU64::new(0),
            claimed_total: AtomicU64::new(0),
            dropped_total: AtomicU64::new(0),
            active_vus: AtomicU64::new(pre_allocated_vus),
            pre_allocated_vus,
            max_vus,
            done: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    pub fn mark_done(&self) {
        self.done.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    pub fn dropped_total(&self) -> u64 {
        self.dropped_total.load(Ordering::Relaxed)
    }

    pub fn active_vus(&self) -> u64 {
        self.active_vus.load(Ordering::Relaxed)
    }

    pub fn max_vus(&self) -> u64 {
        self.max_vus
    }

    pub fn update_due(&self, add_due: u64) {
        if add_due == 0 {
            // Still update active_vus based on backlog.
            self.update_active_vus();
            return;
        }

        // We bound backlog to avoid accumulating a large queue.
        let claimed = self.claimed_total.load(Ordering::Relaxed);
        let scheduled = self.scheduled_total.load(Ordering::Relaxed);
        let backlog = scheduled.saturating_sub(claimed);

        let max_backlog = self.max_vus.max(1);
        let allowed_to_add = max_backlog.saturating_sub(backlog);
        let to_add = add_due.min(allowed_to_add);
        let dropped = add_due.saturating_sub(to_add);

        if to_add != 0 {
            self.scheduled_total.fetch_add(to_add, Ordering::Relaxed);
        }
        if dropped != 0 {
            self.dropped_total.fetch_add(dropped, Ordering::Relaxed);
        }

        self.update_active_vus();
        self.notify.notify_waiters();
    }

    fn update_active_vus(&self) {
        let claimed = self.claimed_total.load(Ordering::Relaxed);
        let scheduled = self.scheduled_total.load(Ordering::Relaxed);
        let backlog = scheduled.saturating_sub(claimed);

        // Simple adaptive policy:
        // - keep at least `pre_allocated_vus`
        // - if backlog exists, raise active VUs to backlog+1 (up to max)
        // - ramp down back to pre-allocated when backlog is 0
        let desired = if backlog == 0 {
            self.pre_allocated_vus
        } else {
            self.pre_allocated_vus.max(backlog.saturating_add(1))
        };

        let desired = desired.clamp(1, self.max_vus);
        self.active_vus.store(desired, Ordering::Relaxed);
    }

    pub async fn claim_next(&self) -> bool {
        loop {
            if self.is_done() {
                let claimed = self.claimed_total.load(Ordering::Relaxed);
                let scheduled = self.scheduled_total.load(Ordering::Relaxed);
                if claimed >= scheduled {
                    return false;
                }
            }

            let claimed = self.claimed_total.load(Ordering::Relaxed);
            let scheduled = self.scheduled_total.load(Ordering::Relaxed);

            if claimed < scheduled {
                if self
                    .claimed_total
                    .compare_exchange_weak(
                        claimed,
                        claimed.saturating_add(1),
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return true;
                }
                continue;
            }

            self.notify.notified().await;
        }
    }

    pub async fn wait_for_update(&self) {
        self.notify.notified().await;
    }
}
