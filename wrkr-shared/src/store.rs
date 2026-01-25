use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::Barrier;
use tokio::sync::watch;

#[derive(Debug, Default)]
pub struct SharedStore {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    values: HashMap<String, Arc<wrkr_value::Value>>,
    notifies: HashMap<String, watch::Sender<u64>>,
    barriers: HashMap<String, BarrierEntry>,
}

#[derive(Debug, Clone)]
struct BarrierEntry {
    parties: usize,
    barrier: Arc<Barrier>,
}

impl SharedStore {
    pub fn set(&self, key: &str, value: wrkr_value::Value) {
        let notify = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            inner.values.insert(key.to_string(), Arc::new(value));
            inner.notifies.get(key).cloned()
        };

        if let Some(notify) = notify {
            let next = notify.borrow().wrapping_add(1);
            let _ = notify.send(next);
        }
    }

    pub fn get(&self, key: &str) -> Option<Arc<wrkr_value::Value>> {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.values.get(key).cloned()
    }

    pub fn delete(&self, key: &str) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.values.remove(key);
    }

    pub fn incr(&self, key: &str, delta: i64) -> i64 {
        let notify = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            let cur = inner.values.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
            let next = cur.saturating_add(delta);
            inner
                .values
                .insert(key.to_string(), Arc::new(wrkr_value::Value::I64(next)));
            inner.notifies.get(key).cloned()
        };

        if let Some(notify) = notify {
            let next = notify.borrow().wrapping_add(1);
            let _ = notify.send(next);
        }

        self.get_counter(key)
    }

    pub fn get_counter(&self, key: &str) -> i64 {
        self.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
    }

    pub async fn wait_for_key(&self, key: &str) {
        let mut rx = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            if inner.values.contains_key(key) {
                return;
            }

            let sender = inner
                .notifies
                .entry(key.to_string())
                .or_insert_with(|| {
                    let (tx, _rx) = watch::channel(0u64);
                    tx
                })
                .clone();

            sender.subscribe()
        };

        loop {
            if self.get(key).is_some() {
                return;
            }

            // If all receivers are dropped, `changed()` returns Err. In that case, we just keep
            // looping and checking the key.
            if rx.changed().await.is_err() {
                tokio::task::yield_now().await;
            }
        }
    }

    pub async fn barrier_wait(&self, name: &str, parties: usize) -> Result<(), SharedBarrierError> {
        if parties == 0 {
            return Err(SharedBarrierError::InvalidParties);
        }

        let barrier = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            match inner.barriers.get(name) {
                None => {
                    let entry = BarrierEntry {
                        parties,
                        barrier: Arc::new(Barrier::new(parties)),
                    };
                    inner.barriers.insert(name.to_string(), entry.clone());
                    entry.barrier
                }
                Some(entry) => {
                    if entry.parties != parties {
                        return Err(SharedBarrierError::PartiesMismatch {
                            expected: entry.parties,
                            got: parties,
                        });
                    }
                    entry.barrier.clone()
                }
            }
        };

        barrier.wait().await;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SharedBarrierError {
    #[error("invalid barrier parties (must be > 0)")]
    InvalidParties,

    #[error("barrier parties mismatch (expected {expected}, got {got})")]
    PartiesMismatch { expected: usize, got: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[test]
    fn set_get_delete_roundtrip() {
        let store = SharedStore::default();

        assert!(store.get("missing").is_none());

        store.set("a", wrkr_value::Value::Bool(true));
        match store.get("a") {
            Some(v) => assert_eq!(&*v, &wrkr_value::Value::Bool(true)),
            None => panic!("expected key a"),
        }

        store.set("b", wrkr_value::Value::String(Arc::<str>::from("hello")));
        match store.get("b") {
            Some(v) => assert_eq!(&*v, &wrkr_value::Value::String(Arc::<str>::from("hello"))),
            None => panic!("expected key b"),
        }

        store.delete("b");
        assert!(store.get("b").is_none());
    }

    #[test]
    fn counter_incr_overwrites_non_counter() {
        let store = SharedStore::default();

        assert_eq!(store.get_counter("c"), 0);
        assert_eq!(store.incr("c", 1), 1);
        assert_eq!(store.incr("c", 41), 42);
        assert_eq!(store.get_counter("c"), 42);

        // Non-i64 values are treated as 0 when incrementing.
        store.set("x", wrkr_value::Value::Bool(true));
        assert_eq!(store.get_counter("x"), 0);
        assert_eq!(store.incr("x", 5), 5);
        assert_eq!(store.get_counter("x"), 5);
    }

    #[tokio::test]
    async fn wait_for_key_unblocks_on_set() {
        let store = Arc::new(SharedStore::default());

        let waiter = {
            let store = store.clone();
            tokio::spawn(async move {
                store.wait_for_key("k").await;
                store.get("k")
            })
        };

        tokio::task::yield_now().await;
        store.set("k", wrkr_value::Value::I64(123));

        let got = match timeout(std::time::Duration::from_secs(1), waiter).await {
            Ok(join) => match join {
                Ok(v) => match v {
                    Some(v) => v,
                    None => panic!("expected key"),
                },
                Err(err) => panic!("waiter task panicked: {err}"),
            },
            Err(_) => panic!("wait_for_key timed out"),
        };
        assert_eq!(&*got, &wrkr_value::Value::I64(123));
    }

    #[tokio::test]
    async fn wait_for_key_unblocks_on_incr() {
        let store = Arc::new(SharedStore::default());

        let waiter = {
            let store = store.clone();
            tokio::spawn(async move {
                store.wait_for_key("ctr").await;
                store.get_counter("ctr")
            })
        };

        tokio::task::yield_now().await;
        assert_eq!(store.incr("ctr", 7), 7);

        let got = match timeout(std::time::Duration::from_secs(1), waiter).await {
            Ok(join) => match join {
                Ok(v) => v,
                Err(err) => panic!("waiter task panicked: {err}"),
            },
            Err(_) => panic!("wait_for_key timed out"),
        };
        assert_eq!(got, 7);
    }

    #[tokio::test]
    async fn barrier_waits_for_parties() {
        let store = Arc::new(SharedStore::default());

        let a = {
            let store = store.clone();
            tokio::spawn(async move { store.barrier_wait("b", 2).await })
        };
        let b = {
            let store = store.clone();
            tokio::spawn(async move { store.barrier_wait("b", 2).await })
        };

        match timeout(std::time::Duration::from_secs(1), async {
            let ra = match a.await {
                Ok(v) => v,
                Err(err) => panic!("task a panicked: {err}"),
            };
            if let Err(err) = ra {
                panic!("task a failed: {err}");
            }

            let rb = match b.await {
                Ok(v) => v,
                Err(err) => panic!("task b panicked: {err}"),
            };
            if let Err(err) = rb {
                panic!("task b failed: {err}");
            }
        })
        .await
        {
            Ok(()) => {}
            Err(_) => panic!("barrier timed out"),
        }
    }

    #[tokio::test]
    async fn barrier_rejects_invalid_parties() {
        let store = SharedStore::default();
        match store.barrier_wait("b", 0).await {
            Ok(()) => panic!("expected error"),
            Err(err) => assert!(matches!(err, SharedBarrierError::InvalidParties)),
        }
    }

    #[tokio::test]
    async fn barrier_rejects_parties_mismatch() {
        let store = Arc::new(SharedStore::default());

        // Create barrier with parties=2.
        let a = {
            let store = store.clone();
            tokio::spawn(async move { store.barrier_wait("b", 2).await })
        };
        let b = {
            let store = store.clone();
            tokio::spawn(async move { store.barrier_wait("b", 2).await })
        };

        match timeout(std::time::Duration::from_secs(1), async {
            let ra = match a.await {
                Ok(v) => v,
                Err(err) => panic!("task a panicked: {err}"),
            };
            if let Err(err) = ra {
                panic!("task a failed: {err}");
            }

            let rb = match b.await {
                Ok(v) => v,
                Err(err) => panic!("task b panicked: {err}"),
            };
            if let Err(err) = rb {
                panic!("task b failed: {err}");
            }
        })
        .await
        {
            Ok(()) => {}
            Err(_) => panic!("barrier timed out"),
        }

        // Re-using same name with different parties should fail.
        match store.barrier_wait("b", 3).await {
            Ok(()) => panic!("expected error"),
            Err(err) => assert!(matches!(err, SharedBarrierError::PartiesMismatch { .. })),
        }
    }
}
