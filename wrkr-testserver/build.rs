use std::path::{Path, PathBuf};

fn main() {
    let proto = PathBuf::from("../wrkr-lua/tests/scripts/protos/echo.proto");
    println!("cargo:rerun-if-changed={}", proto.display());
    println!("cargo:rerun-if-env-changed=PROTOC");

    // External protoc only. Either set `PROTOC=/path/to/protoc` or ensure `protoc` is on PATH.
    let protoc = std::env::var_os("PROTOC").filter(|v| !v.is_empty());
    if protoc.is_none() {
        match std::process::Command::new("protoc")
            .arg("--version")
            .output()
        {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let exit = out.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&out.stderr);
                panic!(
                    "protoc is required to build wrkr-testserver but PATH 'protoc' failed (exit={exit}): {stderr}\n\
                     Install protoc (protobuf compiler) or set PROTOC=/path/to/protoc"
                );
            }
            Err(e) => {
                panic!(
                    "protoc is required to build wrkr-testserver but was not found on PATH: {e}\n\
                     Install protoc (protobuf compiler) or set PROTOC=/path/to/protoc"
                );
            }
        }
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
