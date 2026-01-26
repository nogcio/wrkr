from __future__ import annotations

import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from shutil import which

from wrkr_tools_common.repo import RepoError, find_repo_root
from .env import format_env_templates
from .errors import ProfileError
from .session import build_profiling, run_warmup, testserver_targets


@dataclass(frozen=True, slots=True)
class ProfileConfig:
    """Configuration for a profiling run."""

    sample_duration_seconds: int
    load_duration: str
    vus: int
    script: str
    pre_sample_sleep_seconds: int = 0
    # Environment variables passed to `wrkr run` as `--env KEY=VALUE`.
    # Values can reference server-provided placeholders: {BASE_URL} and {GRPC_TARGET}.
    env_templates: tuple[str, ...] = ()


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

    if sys.platform not in {"darwin", "linux"}:
        raise ProfileError(
            "wrkr-tools-profile currently supports macOS ('sample') and Linux ('perf') only."
        )

    build_profiling(root)

    with testserver_targets(root=root) as targets:
        grpc_target = targets.grpc_target
        print(f"GRPC_TARGET={grpc_target}")

        env_kv = format_env_templates(
            cfg.env_templates,
            base_url=targets.base_url,
            grpc_target=grpc_target,
        )

        # Warmup.
        run_warmup(root, cfg.script, env_kv)

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

        env_args: list[str] = []
        for kv in env_kv:
            env_args.extend(["--env", kv])

        wrkr_proc = subprocess.Popen(
            [
                str(wrkr_bin),
                "run",
                cfg.script,
                "--duration",
                cfg.load_duration,
                "--vus",
                str(cfg.vus),
                *env_args,
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

        # Capture stacks.
        if sys.platform == "darwin":
            _run_macos_sample(
                pid=wrkr_proc.pid,
                duration_seconds=cfg.sample_duration_seconds,
                out_file=sample_out,
            )
        else:
            _run_linux_perf(
                pid=wrkr_proc.pid,
                duration_seconds=cfg.sample_duration_seconds,
                out_prefix=sample_out,
            )

        wrkr_proc.wait()

        print("\nProfiling artifacts written under tmp/.")

        # Server is managed by the context manager.


def _run_macos_sample(*, pid: int, duration_seconds: int, out_file: Path) -> None:
    if which("sample") is None:
        raise ProfileError("Missing required tool: 'sample' (macOS only).")

    print(
        f"Sampling for {duration_seconds}s via macOS 'sample' (output: {out_file})...",
        flush=True,
    )
    result = subprocess.run(
        [
            "sample",
            str(pid),
            str(duration_seconds),
            "-file",
            str(out_file),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )

    if result.returncode != 0:
        raise ProfileError(
            f"'sample' failed (exit {result.returncode}). "
            f"Try running: sample {pid} {duration_seconds} -file {out_file}"
        )

    print(f"Sample written to: {out_file}")
    print("Top hint: search for 'Call graph:' and 'Heaviest stack' inside that file.")


def _run_linux_perf(*, pid: int, duration_seconds: int, out_prefix: Path) -> None:
    perf = which("perf")
    if perf is None:
        raise ProfileError(
            "Missing required tool: 'perf'. In the devcontainer, install it via apt (linux-tools)."
        )

    perf_data = out_prefix.with_suffix("")
    perf_data = perf_data.with_name(f"{perf_data.name}_perf_{duration_seconds}s.data")
    perf_report = perf_data.with_suffix(".report.txt")
    perf_script = perf_data.with_suffix(".script.txt")
    perf_folded = perf_data.with_suffix(".folded.txt")
    perf_flamegraph = perf_data.with_suffix(".flamegraph.svg")

    perf_data.unlink(missing_ok=True)
    perf_report.unlink(missing_ok=True)
    perf_script.unlink(missing_ok=True)
    perf_folded.unlink(missing_ok=True)
    perf_flamegraph.unlink(missing_ok=True)

    print(
        f"Sampling for {duration_seconds}s via perf (data: {perf_data})...",
        flush=True,
    )

    # Note: perf may require extra privileges in containers.
    record = subprocess.run(
        [
            perf,
            "record",
            "-F",
            "99",
            "-g",
            "--call-graph",
            "dwarf",
            "-p",
            str(pid),
            "-o",
            str(perf_data),
            "--",
            "sleep",
            str(duration_seconds),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )

    if record.returncode != 0:
        hint = ""
        if record.stderr:
            hint = record.stderr.strip()
        raise ProfileError(
            "perf record failed. This often means the container lacks permission to access perf events. "
            "Try running the devcontainer with extra privileges (or adjust host sysctl for perf_event_paranoid). "
            f"Details: {hint}"
        )

    # Produce human-readable artifacts alongside perf.data.
    subprocess.run(
        [perf, "script", "-i", str(perf_data)],
        stdout=perf_script.open("w", encoding="utf-8"),
        stderr=subprocess.DEVNULL,
        check=False,
    )
    subprocess.run(
        [perf, "report", "--stdio", "-i", str(perf_data)],
        stdout=perf_report.open("w", encoding="utf-8"),
        stderr=subprocess.DEVNULL,
        check=False,
    )

    inferno_collapse = which("inferno-collapse-perf")
    inferno_flamegraph = which("inferno-flamegraph")
    if inferno_collapse is not None and inferno_flamegraph is not None:
        # Produce a browser-friendly flamegraph SVG alongside the raw perf artifacts.
        with perf_script.open("r", encoding="utf-8") as in_f, perf_folded.open(
            "w", encoding="utf-8"
        ) as out_f:
            subprocess.run(
                [inferno_collapse],
                stdin=in_f,
                stdout=out_f,
                stderr=subprocess.DEVNULL,
                check=False,
            )

        with perf_folded.open("r", encoding="utf-8") as in_f, perf_flamegraph.open(
            "w", encoding="utf-8"
        ) as out_f:
            subprocess.run(
                [inferno_flamegraph],
                stdin=in_f,
                stdout=out_f,
                stderr=subprocess.DEVNULL,
                check=False,
            )

    print(f"perf data written to: {perf_data}")
    print(f"perf report written to: {perf_report}")
    print(f"perf script written to: {perf_script}")
    if perf_flamegraph.exists():
        print(f"perf flamegraph written to: {perf_flamegraph}")
