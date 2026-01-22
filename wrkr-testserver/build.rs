use std::path::{Path, PathBuf};

fn main() {
    let proto = PathBuf::from("../wrkr-lua/tests/scripts/protos/echo.proto");
    println!("cargo:rerun-if-changed={}", proto.display());
    println!("cargo:rerun-if-env-changed=PROTOC");

    let protoc = match protoc_bin_vendored::protoc_bin_path() {
        Ok(p) => p,
        Err(e) => panic!("failed to resolve vendored protoc: {e}"),
    };
    // SAFETY: build scripts run in a dedicated process; we set PROTOC before invoking tonic-prost-build.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }

    let includes_dir = proto
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    if let Err(e) = tonic_prost_build::configure()
        .build_client(false)
        .compile_protos(
            std::slice::from_ref(&proto),
            std::slice::from_ref(&includes_dir),
        )
    {
        panic!("failed to compile grpc test protos: {e}");
    }
}
