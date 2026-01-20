use std::path::{Path, PathBuf};

use prost::Message as _;
use prost_reflect::DescriptorPool;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to locate protoc binary: {0}")]
    ProtocBin(String),

    #[error("failed to run protoc: {0}")]
    ProtocIo(#[from] std::io::Error),

    #[error("protoc failed (exit={exit}): {stderr}")]
    ProtocFailed { exit: i32, stderr: String },

    #[error("failed to build descriptor pool: {0}")]
    DescriptorPool(#[from] prost_reflect::DescriptorError),

    #[error("failed to decode FileDescriptorSet: {0}")]
    DescriptorDecode(#[from] prost::DecodeError),

    #[error("invalid full method name (expected 'pkg.Service/Method'): {0}")]
    InvalidFullMethod(String),

    #[error("service not found in descriptors: {0}")]
    ServiceNotFound(String),

    #[error("method not found in service '{service}': {method}")]
    MethodNotFound { service: String, method: String },
}

#[derive(Debug, Clone)]
pub struct ProtoSchema {
    pool: DescriptorPool,
}

#[derive(Debug, Clone)]
pub struct GrpcMethod {
    method: prost_reflect::MethodDescriptor,
}

impl GrpcMethod {
    pub(crate) fn descriptor(&self) -> &prost_reflect::MethodDescriptor {
        &self.method
    }
}

impl ProtoSchema {
    pub fn compile_from_proto(proto_file: &Path, include_paths: &[PathBuf]) -> Result<Self> {
        let mut include_paths: Vec<PathBuf> = include_paths.to_vec();

        if let Some(dir) = proto_file.parent() {
            include_paths.push(dir.to_path_buf());
        }

        // Deduplicate while preserving order (tiny input sizes).
        let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        include_paths.retain(|p| seen.insert(p.clone()));

        let protoc =
            protoc_bin_vendored::protoc_bin_path().map_err(|e| Error::ProtocBin(e.to_string()))?;

        let out = tempfile::NamedTempFile::new()?;
        let out_path = out.path().to_path_buf();

        let mut cmd = std::process::Command::new(protoc);
        cmd.arg("--include_imports")
            .arg("--include_source_info")
            .arg(format!("--descriptor_set_out={}", out_path.display()));

        for p in &include_paths {
            cmd.arg("-I").arg(p);
        }

        cmd.arg(proto_file);

        let output = cmd.output()?;
        if !output.status.success() {
            let exit = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(Error::ProtocFailed { exit, stderr });
        }

        let bytes = std::fs::read(out_path)?;
        let fds = prost_types::FileDescriptorSet::decode(bytes.as_slice())?;
        let pool = DescriptorPool::from_file_descriptor_set(fds)?;

        Ok(Self { pool })
    }

    pub fn method(&self, full_method: &str) -> Result<GrpcMethod> {
        let (service_name, method_name) = full_method
            .split_once('/')
            .ok_or_else(|| Error::InvalidFullMethod(full_method.to_string()))?;

        let service = self
            .pool
            .get_service_by_name(service_name)
            .ok_or_else(|| Error::ServiceNotFound(service_name.to_string()))?;

        let method = service
            .methods()
            .find(|m| m.name() == method_name)
            .ok_or_else(|| Error::MethodNotFound {
                service: service_name.to_string(),
                method: method_name.to_string(),
            })?;

        Ok(GrpcMethod { method })
    }
}
