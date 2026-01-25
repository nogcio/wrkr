from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .exec import ExecError, run_checked_streaming
from .ui import RunUI


class BuildError(RuntimeError):
    """Raised when the Rust build step fails."""


@dataclass(frozen=True, slots=True)
class BuildPlan:
    """
    Build settings for required Rust binaries.

    This tool expects `wrkr` and `wrkr-testserver` to exist under:
      {root}/target/release/{bin}
    """

    root: Path
    native: bool = True

    def rustflags(self) -> str | None:
        # Keep behavior aligned with the prior tool: `-C target-cpu=native` when enabled.
        return "-C target-cpu=native" if self.native else None


def build_binaries(plan: BuildPlan, *, ui: RunUI) -> None:
    """
    Build the required release binaries using cargo.

    Builds:
      - wrkr-testserver (package wrkr-testserver, bin wrkr-testserver)
      - wrkr (workspace bin wrkr)

    Notes
    -----
    We build as two separate cargo invocations, matching the old behavior and keeping
    error messages focused.

    Raises
    ------
    BuildError
        If any cargo invocation fails.
    """
    root = plan.root.resolve()
    if not root.exists():
        raise BuildError(f"WRKR root does not exist: {root}")

    env: dict[str, str] = {}
    rf = plan.rustflags()
    if rf:
        env["RUSTFLAGS"] = rf

    ui.log("Building release binaries...", style="bold")

    try:
        # Build wrkr-testserver first (needed for server startup).
        with ui.step("build: wrkr-testserver"):
            ui.set_current_command(
                label="cargo",
                argv=[
                    "cargo",
                    "build",
                    "--release",
                    "-p",
                    "wrkr-testserver",
                    "--bin",
                    "wrkr-testserver",
                ],
                cwd=root,
                env=env or None,
            )
            run_checked_streaming(
                [
                    "cargo",
                    "build",
                    "--release",
                    "-p",
                    "wrkr-testserver",
                    "--bin",
                    "wrkr-testserver",
                ],
                cwd=root,
                env=env or None,
                label=None,
                on_stdout_line=ui.tail,
                on_stderr_line=lambda line: ui.tail(line, style="dim"),
            )

        # Build wrkr binary (used to run Lua scripts).
        with ui.step("build: wrkr"):
            ui.set_current_command(
                label="cargo",
                argv=["cargo", "build", "--release", "--bin", "wrkr"],
                cwd=root,
                env=env or None,
            )
            run_checked_streaming(
                ["cargo", "build", "--release", "--bin", "wrkr"],
                cwd=root,
                env=env or None,
                label=None,
                on_stdout_line=ui.tail,
                on_stderr_line=lambda line: ui.tail(line, style="dim"),
            )

    except ExecError as e:
        raise BuildError(str(e)) from e
