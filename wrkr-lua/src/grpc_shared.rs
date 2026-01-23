use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use tokio::sync::OnceCell;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadSpec {
    include_paths: Vec<PathBuf>,
    proto_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConnectSpec {
    target: String,
    timeout: Option<Duration>,
    tls: Option<ConnectSpecTls>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConnectSpecTls {
    ca_pem: Option<Vec<u8>>,
    identity_pem: Option<Vec<u8>>,
    identity_key_pem: Option<Vec<u8>>,
    domain_name: Option<String>,
    insecure_skip_verify: bool,
}

#[derive(Debug)]
pub(crate) struct SharedGrpcClient {
    pool_size: usize,

    load_lock: Mutex<()>,
    load_spec: OnceLock<LoadSpec>,
    schema: OnceLock<Arc<wrkr_core::ProtoSchema>>,
    methods: RwLock<HashMap<Arc<str>, Arc<wrkr_core::GrpcMethod>>>,

    connect_spec: Mutex<Option<ConnectSpec>>,
    client: OnceCell<Arc<wrkr_core::GrpcClient>>,
}

impl SharedGrpcClient {
    fn new(pool_size: usize) -> Self {
        Self {
            pool_size: pool_size.max(1),
            load_lock: Mutex::new(()),
            load_spec: OnceLock::new(),
            schema: OnceLock::new(),
            methods: RwLock::new(HashMap::new()),
            connect_spec: Mutex::new(None),
            client: OnceCell::new(),
        }
    }

    pub(crate) fn load(
        &self,
        include_paths: Vec<PathBuf>,
        proto_file: PathBuf,
    ) -> Result<(), String> {
        let _guard = self.load_lock.lock().unwrap_or_else(|p| p.into_inner());

        let spec = LoadSpec {
            include_paths,
            proto_file,
        };

        if let Some(existing) = self.load_spec.get() {
            if existing != &spec {
                return Err(
                    "grpc client: load() called multiple times with different arguments"
                        .to_string(),
                );
            }
            return Ok(());
        }

        let schema =
            wrkr_core::ProtoSchema::compile_from_proto(&spec.proto_file, &spec.include_paths)
                .map_err(|e| e.to_string())?;

        // First successful load wins.
        let _ = self.load_spec.set(spec);
        let _ = self.schema.set(Arc::new(schema));

        self.methods
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .clear();

        Ok(())
    }

    pub(crate) fn method(&self, full_method: &str) -> Result<Arc<wrkr_core::GrpcMethod>, String> {
        if let Some(existing) = self
            .methods
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(full_method)
            .cloned()
        {
            return Ok(existing);
        }

        let schema = self
            .schema
            .get()
            .ok_or_else(|| "grpc client: call load() first".to_string())?;

        let m = schema.method(full_method).map_err(|e| e.to_string())?;
        let m = Arc::new(m);

        let key: Arc<str> = Arc::from(full_method);
        let mut guard = self.methods.write().unwrap_or_else(|p| p.into_inner());
        Ok(guard.entry(key).or_insert_with(|| m.clone()).clone())
    }

    pub(crate) async fn connect(
        &self,
        target: String,
        opts: wrkr_core::GrpcConnectOptions,
    ) -> Result<(), String> {
        let spec = ConnectSpec {
            target: target.clone(),
            timeout: opts.timeout,
            tls: opts.tls.as_ref().map(|tls| ConnectSpecTls {
                ca_pem: tls.ca_pem.clone(),
                identity_pem: tls.identity_pem.clone(),
                identity_key_pem: tls.identity_key_pem.clone(),
                domain_name: tls.domain_name.clone(),
                insecure_skip_verify: tls.insecure_skip_verify,
            }),
        };

        {
            let mut guard = self.connect_spec.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(existing) = guard.as_ref() {
                if existing != &spec {
                    return Err(
                        "grpc client: connect() called multiple times with different arguments"
                            .to_string(),
                    );
                }
            } else {
                *guard = Some(spec);
            }
        }

        let pool_size = self.pool_size;
        self.client
            .get_or_try_init(|| async move {
                let client =
                    wrkr_core::GrpcClient::connect_pooled(&target, opts, pool_size).await?;
                Ok::<Arc<wrkr_core::GrpcClient>, wrkr_core::GrpcError>(Arc::new(client))
            })
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub(crate) fn client(&self) -> Option<Arc<wrkr_core::GrpcClient>> {
        self.client.get().cloned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RegistryKey {
    run_id: u64,
    pool_size: usize,
}

static SHARED: OnceLock<Mutex<HashMap<RegistryKey, Arc<SharedGrpcClient>>>> = OnceLock::new();

pub(crate) fn default_pool_size(max_vus: u64) -> usize {
    let vus = max_vus as usize;
    // Aim for roughly 8 VUs per channel, but clamp the pool size between 16 and 64
    // connections so we keep a reasonable lower bound for low VU counts and never
    // explode connection count for very high VU counts.
    (vus / 8).clamp(16, 64).max(1)
}

pub(crate) fn get_or_create(
    stats: &Arc<wrkr_core::runner::RunStats>,
    pool_size: usize,
) -> Arc<SharedGrpcClient> {
    let key = RegistryKey {
        run_id: stats.run_id(),
        pool_size: pool_size.max(1),
    };

    let map = SHARED.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|p| p.into_inner());
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(SharedGrpcClient::new(key.pool_size)))
        .clone()
}
