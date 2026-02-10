use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use wrkr_metrics::Registry;

#[derive(Debug, Clone)]
pub(crate) struct PrometheusPushConfig {
    pub pushgateway: wrkr_metrics::prometheus::pushgateway::PushgatewayConfig,
    pub interval: Duration,
}

#[derive(Debug)]
struct State {
    last_push: Option<Instant>,
    in_flight: bool,
    last_error: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct PrometheusPusher {
    metrics: Arc<Registry>,
    cfg: PrometheusPushConfig,
    state: Arc<Mutex<State>>,
    handle: tokio::runtime::Handle,
}

impl PrometheusPusher {
    pub(crate) fn new(metrics: Arc<Registry>, cfg: PrometheusPushConfig) -> Self {
        Self {
            metrics,
            cfg,
            state: Arc::new(Mutex::new(State {
                last_push: None,
                in_flight: false,
                last_error: None,
            })),
            handle: tokio::runtime::Handle::current(),
        }
    }

    pub(crate) fn progress_fn(&self) -> wrkr_core::ProgressFn {
        let this = self.clone();
        Arc::new(move |_u: wrkr_core::ProgressUpdate| {
            this.maybe_push();
        })
    }

    pub(crate) async fn push_final(&self) {
        let metrics = self.metrics.clone();
        let cfg = self.cfg.pushgateway.clone();
        let cfg_for_push = cfg.clone();
        let join = tokio::task::spawn_blocking(move || {
            let _ = wrkr_metrics::prometheus::pushgateway::push_registry(
                metrics.as_ref(),
                &cfg_for_push,
            );
        });

        // Best-effort: avoid hanging shutdown forever.
        let _ = tokio::time::timeout(cfg.timeout + Duration::from_secs(1), join).await;
    }

    fn maybe_push(&self) {
        let now = Instant::now();
        let mut st = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if st.in_flight {
            return;
        }

        if let Some(last) = st.last_push
            && now.duration_since(last) < self.cfg.interval
        {
            return;
        }

        st.in_flight = true;
        st.last_push = Some(now);
        drop(st);

        let state = self.state.clone();
        let metrics = self.metrics.clone();
        let cfg = self.cfg.pushgateway.clone();

        self.handle.spawn_blocking(move || {
            let res = wrkr_metrics::prometheus::pushgateway::push_registry(metrics.as_ref(), &cfg);

            let mut st = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            st.in_flight = false;

            if let Err(e) = res {
                let should_log = match st.last_error {
                    None => true,
                    Some(prev) => Instant::now().duration_since(prev) >= Duration::from_secs(10),
                };
                if should_log {
                    st.last_error = Some(Instant::now());
                    eprintln!("pushgateway: push failed: {e}");
                }
            }
        });
    }
}

pub(crate) fn default_run_id() -> String {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    format!("{pid}-{nanos}")
}

pub(crate) fn parse_grouping_labels(items: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for item in items {
        let Some((k, v)) = item.split_once('=') else {
            return Err(format!("invalid label '{item}' (expected KEY=VALUE)"));
        };
        if k.trim().is_empty() {
            return Err(format!("invalid label '{item}' (empty key)"));
        }
        out.push((k.trim().to_string(), v.to_string()));
    }
    Ok(out)
}
