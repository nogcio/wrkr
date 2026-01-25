from __future__ import annotations

import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from wrkr_tools_common.repo import RepoError, find_repo_root
from wrkr_tools_common.server import ServerError, TestServer


class ProfileError(RuntimeError):
    """Raised when profiling setup or execution fails."""


@dataclass(frozen=True, slots=True)
class ProfileConfig:
    """Configuration for a profiling run."""

    sample_duration_seconds: int
    load_duration: str
    vus: int
    script: str
    pre_sample_sleep_seconds: int = 0


def run_profile(cfg: ProfileConfig) -> None:
    """
    Run a profiling session.

    Steps:
    1. Build wrkr binaries in profiling mode.
    2. Start wrkr-testserver and wait for GRPC_TARGET.
    3. Warmup run.
    4. Run wrkr in background and sample its stacks.
    5. Write sample output to tmp/.
    """
    try:
        root = find_repo_root()
    except RepoError as e:
        raise ProfileError(str(e)) from e
    print(f"Repo root: {root}")

    if sys.platform != "darwin":
        raise ProfileError("wrkr-tools-profile currently supports macOS only (requires 'sample').")

    _build_profiling(root)

    server_bin = root / "target" / "profiling" / "wrkr-testserver"
    if not server_bin.exists():
        raise ProfileError(f"Missing wrkr-testserver binary: {server_bin}")

    try:
        server = TestServer.start(
            root=root,
            server_bin=server_bin,
            on_log=lambda m: print(m, flush=True),
        )
    except ServerError as e:
        raise ProfileError(str(e)) from e

    with server:
        try:
            targets = server.wait_for_targets(
                timeout_s=10.0,
                on_log=lambda m: print(m, flush=True),
            )
        except ServerError as e:
            raise ProfileError(str(e)) from e

        grpc_target = targets.grpc_target
        print(f"GRPC_TARGET={grpc_target}")

        # Warmup.
        _run_warmup(root, cfg.script, grpc_target)

        # Prepare output file.
        tmp_dir = root / "tmp"
        tmp_dir.mkdir(parents=True, exist_ok=True)

        script_name = Path(cfg.script).stem
        sample_out = tmp_dir / f"{script_name}_sample_{cfg.sample_duration_seconds}s.txt"
        sample_out.unlink(missing_ok=True)

        # Run wrkr in background.
        wrkr_bin = root / "target" / "profiling" / "wrkr"
        if not wrkr_bin.exists():
            raise ProfileError(f"Missing wrkr binary: {wrkr_bin}")

        print(
            f"Running wrkr (vus={cfg.vus}, duration={cfg.load_duration}, script={cfg.script})...",
            flush=True,
        )
        wrkr_proc = subprocess.Popen(
            [
                str(wrkr_bin),
                "run",
                cfg.script,
                "--duration",
                cfg.load_duration,
                "--vus",
                str(cfg.vus),
                "--env",
                f"GRPC_TARGET={grpc_target}",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            cwd=str(root),
        )

        if cfg.pre_sample_sleep_seconds > 0:
            print(
                f"Sleeping {cfg.pre_sample_sleep_seconds}s before sampling (to avoid startup skew)...",
                flush=True,
            )
            time.sleep(cfg.pre_sample_sleep_seconds)

        # Run sample.
        print(
            f"Sampling for {cfg.sample_duration_seconds}s (output: {sample_out})...",
            flush=True,
        )
        sample_result = subprocess.run(
            [
                "sample",
                str(wrkr_proc.pid),
                str(cfg.sample_duration_seconds),
                "-file",
                str(sample_out),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )

        wrkr_proc.wait()

        if sample_result.returncode != 0:
            raise ProfileError(
                f"'sample' failed (exit {sample_result.returncode}). "
                f"Try running: sample {wrkr_proc.pid} {cfg.sample_duration_seconds} -file {sample_out}"
            )

        print(f"\nSample written to: {sample_out}")
        print("Top hint: search for 'Call graph:' and 'Heaviest stack' inside that file.")

        # Server is managed by the context manager.


def _build_profiling(root: Path) -> None:
    """Build wrkr binaries in profiling mode."""
    print("Building profiling binaries (cargo build --profile profiling)...", flush=True)
    result = subprocess.run(
        ["cargo", "build", "--profile", "profiling"],
        cwd=str(root),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr)
        raise ProfileError(f"cargo build failed (exit {result.returncode})")


def _run_warmup(root: Path, script: str, grpc_target: str) -> None:
    """Run a quick warmup to avoid one-time startup costs in the sample."""
    print("Running warmup (1s, 64 vus)...", flush=True)
    wrkr_bin = root / "target" / "profiling" / "wrkr"
    subprocess.run(
        [
            str(wrkr_bin),
            "run",
            script,
            "--duration",
            "1s",
            "--vus",
            "64",
            "--env",
            f"GRPC_TARGET={grpc_target}",
        ],
        cwd=str(root),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=True,
    )
