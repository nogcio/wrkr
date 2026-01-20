use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

pub(crate) struct HumanProgress {
    inner: Mutex<Inner>,
}

impl HumanProgress {
    pub(crate) fn new() -> Self {
        let multi = MultiProgress::new();
        multi.set_draw_target(ProgressDrawTarget::stderr_with_hz(5));

        Self {
            inner: Mutex::new(Inner {
                multi,
                bars: HashMap::new(),
            }),
        }
    }

    pub(crate) fn update(
        &self,
        scenario: &str,
        total_duration_opt: Option<Duration>,
        elapsed: Duration,
        message: String,
    ) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let pb = inner.get_or_create_bar(scenario, total_duration_opt);
        pb.set_message(message);

        match total_duration_opt {
            Some(total_d) => {
                let total_ms = total_d.as_millis() as u64;
                let elapsed_ms = elapsed.as_millis() as u64;
                pb.set_length(total_ms);
                pb.set_position(elapsed_ms.min(total_ms));
            }
            None => {
                pb.tick();
            }
        }
    }

    pub(crate) fn finish(&self) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        for (_, b) in inner.bars.drain() {
            b.pb.finish_and_clear();
        }

        let _ = inner.multi.clear();
    }
}

struct Inner {
    multi: MultiProgress,
    bars: HashMap<String, ScenarioProgressBar>,
}

impl Inner {
    fn get_or_create_bar(
        &mut self,
        scenario: &str,
        total_duration_opt: Option<Duration>,
    ) -> &ProgressBar {
        let desired_kind = if total_duration_opt.is_some() {
            ProgressBarKind::Bar
        } else {
            ProgressBarKind::Spinner
        };

        let needs_recreate = self
            .bars
            .get(scenario)
            .is_some_and(|b| b.kind != desired_kind);

        if needs_recreate && let Some(old) = self.bars.remove(scenario) {
            old.pb.finish_and_clear();
        }

        let entry = self
            .bars
            .entry(scenario.to_string())
            .or_insert_with(|| match desired_kind {
                ProgressBarKind::Bar => {
                    let pb = self.multi.add(ProgressBar::new(0));
                    pb.set_style(bar_style());
                    pb.set_prefix(scenario.to_string());
                    ScenarioProgressBar {
                        kind: ProgressBarKind::Bar,
                        pb,
                    }
                }
                ProgressBarKind::Spinner => {
                    let pb = self.multi.add(ProgressBar::new_spinner());
                    pb.set_style(spinner_style());
                    pb.set_prefix(scenario.to_string());
                    pb.enable_steady_tick(Duration::from_millis(120));
                    ScenarioProgressBar {
                        kind: ProgressBarKind::Spinner,
                        pb,
                    }
                }
            });

        &entry.pb
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressBarKind {
    Spinner,
    Bar,
}

struct ScenarioProgressBar {
    kind: ProgressBarKind,
    pb: ProgressBar,
}

fn bar_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix} [ {bar:20.cyan/blue} ] {percent:>3}% {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("█░")
}

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix} {spinner} {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
}
