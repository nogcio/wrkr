use std::sync::Arc;

use futures_util::FutureExt as _;
use futures_util::future::BoxFuture;

pub(crate) struct Subscriber {
    progress: Option<wrkr_core::ProgressFn>,
    finalizer: Option<BoxFuture<'static, ()>>,
}

impl Subscriber {
    #[must_use]
    pub(crate) fn progress_opt(progress: Option<wrkr_core::ProgressFn>) -> Self {
        Self {
            progress,
            finalizer: None,
        }
    }

    #[must_use]
    pub(crate) fn finalizer<Fut>(fut: Fut) -> Self
    where
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        Self {
            progress: None,
            finalizer: Some(fut.boxed()),
        }
    }

    #[must_use]
    pub(crate) fn both<Fut>(progress: wrkr_core::ProgressFn, fut: Fut) -> Self
    where
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        Self {
            progress: Some(progress),
            finalizer: Some(fut.boxed()),
        }
    }

    fn into_parts(
        self,
    ) -> (
        Option<wrkr_core::ProgressFn>,
        Option<BoxFuture<'static, ()>>,
    ) {
        (self.progress, self.finalizer)
    }
}

pub(crate) struct Subscribers {
    progress_fns: Vec<wrkr_core::ProgressFn>,
    finalizers: Vec<BoxFuture<'static, ()>>,
}

impl Subscribers {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            progress_fns: Vec::new(),
            finalizers: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, sub: Subscriber) {
        let (progress, finalizer) = sub.into_parts();
        if let Some(p) = progress {
            self.progress_fns.push(p);
        }
        if let Some(f) = finalizer {
            self.finalizers.push(f);
        }
    }

    #[must_use]
    pub(crate) fn build_progress_fn(&self) -> Option<wrkr_core::ProgressFn> {
        match self.progress_fns.len() {
            0 => None,
            1 => self.progress_fns.first().cloned(),
            _ => {
                let fns = Arc::new(self.progress_fns.clone());
                Some(Arc::new(move |u| {
                    let Some((last, rest)) = fns.split_last() else {
                        return;
                    };
                    for f in rest {
                        (f)(u.clone());
                    }
                    (last)(u);
                }))
            }
        }
    }

    pub(crate) async fn finalize_all(self) {
        for f in self.finalizers {
            f.await;
        }
    }
}

impl Default for Subscribers {
    fn default() -> Self {
        Self::new()
    }
}
