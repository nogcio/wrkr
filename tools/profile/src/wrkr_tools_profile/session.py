from __future__ import annotations

import subprocess
import sys
from contextlib import contextmanager
from pathlib import Path

from wrkr_tools_common.server import ServerError, TestServer

from .errors import ProfileError


def build_profiling(root: Path) -> None:
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


def run_warmup(root: Path, script: str, env_kv: list[str]) -> None:
    """Run a quick warmup to avoid one-time startup costs in the sample."""
    print("Running warmup (1s, 64 vus)...", flush=True)
    wrkr_bin = root / "target" / "profiling" / "wrkr"

    env_args: list[str] = []
    for kv in env_kv:
        env_args.extend(["--env", kv])

    result = subprocess.run(
        [
            str(wrkr_bin),
            "run",
            script,
            "--duration",
            "1s",
            "--vus",
            "64",
            *env_args,
        ],
        cwd=str(root),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )

    if result.returncode != 0:
        print(
            f"Warmup exit code: {result.returncode} (continuing)",
            flush=True,
        )


@contextmanager
def testserver_targets(*, root: Path):
    """Start wrkr-testserver and yield its discovered targets."""
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

        yield targets
