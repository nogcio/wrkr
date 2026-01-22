use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use prost::Message as _;
use prost_reflect::DescriptorPool;
use tonic::codegen::http::uri::PathAndQuery;

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

    #[error("invalid protobuf descriptor: {0}")]
    InvalidDescriptor(String),

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
pub(crate) enum GrpcFieldShape {
    Scalar {
        kind: prost_reflect::Kind,
    },
    List {
        kind: prost_reflect::Kind,
    },
    Map {
        key_kind: prost_reflect::Kind,
        value_kind: prost_reflect::Kind,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct GrpcInputFieldMeta {
    pub(crate) field: prost_reflect::FieldDescriptor,
    pub(crate) shape: GrpcFieldShape,
}

#[derive(Debug, Clone)]
pub(crate) struct GrpcOutputFieldMeta {
    pub(crate) field: prost_reflect::FieldDescriptor,
    pub(crate) name: Arc<str>,
}

#[derive(Debug, Clone)]
pub struct GrpcMethod {
    path: PathAndQuery,
    input: prost_reflect::MessageDescriptor,
    input_fields: HashMap<Arc<str>, GrpcInputFieldMeta>,
    output: prost_reflect::MessageDescriptor,
    output_fields: Vec<GrpcOutputFieldMeta>,
}

impl GrpcMethod {
    pub(crate) fn path(&self) -> &PathAndQuery {
        &self.path
    }

    pub(crate) fn input_desc(&self) -> &prost_reflect::MessageDescriptor {
        &self.input
    }

    pub(crate) fn input_fields(&self) -> &HashMap<Arc<str>, GrpcInputFieldMeta> {
        &self.input_fields
    }

    pub(crate) fn output_desc(&self) -> &prost_reflect::MessageDescriptor {
        &self.output
    }

    pub(crate) fn output_fields(&self) -> &[GrpcOutputFieldMeta] {
        self.output_fields.as_slice()
    }
}

impl ProtoSchema {
    fn resolve_protoc() -> Result<std::ffi::OsString> {
        if let Some(p) = std::env::var_os("PROTOC").filter(|v| !v.is_empty()) {
            return Ok(p);
        }

        if let Some(p) = Self::bundled_protoc_path() {
            return Ok(p.into_os_string());
        }

        if Self::path_protoc_is_runnable() {
            return Ok(std::ffi::OsString::from("protoc"));
        }

        Err(Error::ProtocBin(
            "no runnable protoc found; install protoc and ensure it's on PATH, set PROTOC=/path/to/protoc, or place protoc next to the wrkr binary"
                .to_string(),
        ))
    }

    fn bundled_protoc_path() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let exe_dir = exe.parent()?;

        let filename = if cfg!(windows) {
            "protoc.exe"
        } else {
            "protoc"
        };
        let candidate = exe_dir.join(filename);
        if !candidate.is_file() {
            return None;
        }

        if Self::protoc_is_runnable(&candidate) {
            Some(candidate)
        } else {
            None
        }
    }

    fn bundled_protoc_include_dir() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let exe_dir = exe.parent()?;

        // When distributing protoc, we also ship the well-known-types protos.
        // This is needed for imports like "google/protobuf/timestamp.proto".
        let candidate = exe_dir.join("protoc-include");
        let sentinel = candidate.join("google").join("protobuf").join("any.proto");

        if sentinel.is_file() {
            Some(candidate)
        } else {
            None
        }
    }

    fn protoc_is_runnable(path: &Path) -> bool {
        match std::process::Command::new(path).arg("--version").output() {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    fn path_protoc_is_runnable() -> bool {
        match std::process::Command::new("protoc")
            .arg("--version")
            .output()
        {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    pub fn compile_from_proto(proto_file: &Path, include_paths: &[PathBuf]) -> Result<Self> {
        let mut include_paths: Vec<PathBuf> = include_paths.to_vec();

        if let Some(dir) = proto_file.parent() {
            include_paths.push(dir.to_path_buf());
        }

        if let Some(wkt_dir) = Self::bundled_protoc_include_dir() {
            include_paths.push(wkt_dir);
        }

        // Deduplicate while preserving order (tiny input sizes).
        let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        include_paths.retain(|p| seen.insert(p.clone()));

        let protoc = Self::resolve_protoc()?;

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

        let path = PathAndQuery::from_maybe_shared(bytes::Bytes::from(format!(
            "/{service_name}/{method_name}"
        )))
        .map_err(|_| Error::InvalidFullMethod(full_method.to_string()))?;

        fn build_shape(field: &prost_reflect::FieldDescriptor) -> Result<GrpcFieldShape> {
            if field.is_map() {
                let prost_reflect::Kind::Message(entry_desc) = field.kind() else {
                    return Err(Error::InvalidDescriptor(
                        "map field did not have message kind".to_string(),
                    ));
                };

                let key_kind = entry_desc
                    .get_field_by_name("key")
                    .ok_or_else(|| {
                        Error::InvalidDescriptor("invalid map entry: missing key".to_string())
                    })?
                    .kind();
                let value_kind = entry_desc
                    .get_field_by_name("value")
                    .ok_or_else(|| {
                        Error::InvalidDescriptor("invalid map entry: missing value".to_string())
                    })?
                    .kind();

                return Ok(GrpcFieldShape::Map {
                    key_kind,
                    value_kind,
                });
            }

            let kind = field.kind();
            if field.is_list() {
                Ok(GrpcFieldShape::List { kind })
            } else {
                Ok(GrpcFieldShape::Scalar { kind })
            }
        }

        let input = method.input();
        let mut input_fields: HashMap<Arc<str>, GrpcInputFieldMeta> =
            HashMap::with_capacity(input.fields().len());
        for f in input.fields() {
            let name = Arc::<str>::from(f.name());
            input_fields.insert(
                name,
                GrpcInputFieldMeta {
                    shape: build_shape(&f)?,
                    field: f,
                },
            );
        }

        let output = method.output();
        let mut output_fields: Vec<GrpcOutputFieldMeta> = Vec::with_capacity(output.fields().len());
        for f in output.fields() {
            let name = Arc::<str>::from(f.name());
            output_fields.push(GrpcOutputFieldMeta { field: f, name });
        }

        Ok(GrpcMethod {
            input,
            input_fields,
            output,
            output_fields,
            path,
        })
    }
}
