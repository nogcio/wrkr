from __future__ import annotations

import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from shutil import which

from wrkr_tools_common.repo import RepoError, find_repo_root

from .errors import ProfileError
from .env import format_env_templates
from .profile import ProfileConfig
from .session import build_profiling, run_warmup, testserver_targets


@dataclass(frozen=True, slots=True)
class SamplyConfig:
    """Configuration for samply output / UI behavior."""

    output_path: Path | None = None
    open_ui: bool = True


def _check_linux_perf_event_paranoid() -> None:
    if sys.platform != "linux":
        return

    paranoid_path = Path("/proc/sys/kernel/perf_event_paranoid")
    try:
        raw = paranoid_path.read_text(encoding="utf-8").strip()
        value = int(raw)
    except (OSError, ValueError):
        return

    # samply uses perf events; typical requirement is <= 1 for non-root.
    if value > 1:
        raise ProfileError(
            "samply requires perf events access, but kernel.perf_event_paranoid is set to "
            f"{value} (needs 1 or lower for non-root).\n"
            "Try:\n"
            "  echo '1' | sudo tee /proc/sys/kernel/perf_event_paranoid\n"
            "Or run the container with appropriate privileges / sysctl." 
        )


def run_samply_profile(cfg: ProfileConfig, *, samply: SamplyConfig | None = None) -> Path:
    """Run a profiling session using `samply record` and return the saved profile path."""
    if which("samply") is None:
        raise ProfileError(
            "Missing required tool: 'samply'. In the devcontainer it should be installed by .devcontainer/postCreate.sh."
        )

    _check_linux_perf_event_paranoid()

    try:
        root = find_repo_root()
    except RepoError as e:
        raise ProfileError(str(e)) from e
    print(f"Repo root: {root}")

    build_profiling(root)

    with testserver_targets(root=root) as targets:
        grpc_target = targets.grpc_target
        print(f"GRPC_TARGET={grpc_target}")

        env_kv = format_env_templates(
            cfg.env_templates,
            base_url=targets.base_url,
            grpc_target=grpc_target,
        )

        run_warmup(root, cfg.script, env_kv)

        wrkr_bin = root / "target" / "profiling" / "wrkr"
        if not wrkr_bin.exists():
            raise ProfileError(f"Missing wrkr binary: {wrkr_bin}")

        env_args: list[str] = []
        for kv in env_kv:
            env_args.extend(["--env", kv])

        wrkr_cmd = [
            str(wrkr_bin),
            "run",
            cfg.script,
            "--duration",
            cfg.load_duration,
            "--vus",
            str(cfg.vus),
            *env_args,
        ]

        tmp_dir = root / "tmp"
        tmp_dir.mkdir(parents=True, exist_ok=True)

        script_name = Path(cfg.script).stem
        default_out = tmp_dir / f"{script_name}_samply_{cfg.sample_duration_seconds}s.profile.json.gz"

        samply_cfg = samply or SamplyConfig()
        out_path = samply_cfg.output_path or default_out
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.unlink(missing_ok=True)

        record_cmd = [
            "samply",
            "record",
            "--duration",
            str(cfg.sample_duration_seconds),
            "--save-only",
            "-n",
            "-o",
            str(out_path),
            "--",
            *wrkr_cmd,
        ]

        print(
            f"Running wrkr under samply (vus={cfg.vus}, load={cfg.load_duration}, sample={cfg.sample_duration_seconds}s)...",
            flush=True,
        )

        result = subprocess.run(
            record_cmd,
            cwd=str(root),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

        if result.returncode != 0:
            hint = (result.stderr or "").strip()
            raise ProfileError(
                f"samply record failed (exit {result.returncode})." + (f" Details: {hint}" if hint else "")
            )

        print(f"\nSamply profile saved to: {out_path}")

        if samply_cfg.open_ui:
            load = subprocess.run(
                ["samply", "load", str(out_path)],
                cwd=str(root),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                check=False,
            )
            if load.returncode != 0:
                print(
                    "WARNING: `samply load` failed; you can load it manually.",
                    file=sys.stderr,
                )
            else:
                # Useful in headless environments: print the URL samply emits.
                if load.stdout:
                    print(load.stdout.rstrip())

        return out_path
