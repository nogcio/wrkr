from __future__ import annotations

import sys
import time
from collections import deque
from collections.abc import Iterable, Mapping, Sequence
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

from rich.console import Console, Group
from rich.live import Live
from rich.progress import (
    BarColumn,
    Progress,
    SpinnerColumn,
    TaskProgressColumn,
    TextColumn,
    TimeElapsedColumn,
)
from rich.text import Text


def _console_for_color_mode(color: str) -> Console:
    mode = color.strip().lower()
    if mode == "always":
        # Emit ANSI color codes even when stdout is not a TTY (useful for piping to `tail`).
        return Console(force_terminal=True)
    if mode == "never":
        return Console(no_color=True)
    if mode == "auto":
        return Console()
    raise ValueError(f"Invalid color mode: {color!r} (expected auto|always|never)")


def _is_interactive_default() -> bool:
    # Docker-like behavior: Live updates only when stdout is a TTY.
    # When not a TTY (CI logs / redirected), fall back to plain lines.
    try:
        return sys.stdout.isatty()
    except Exception:
        return False


@dataclass(frozen=True, slots=True)
class CommandInfo:
    label: str
    argv: Sequence[str]
    cwd: Path | None
    env: Mapping[str, str] | None


class RunUI:
    """
    Single-mode, docker-like UI for the compare-perf tool.

    - If stdout is a TTY: uses Rich Live to continuously render:
      progress bars + current command + last N tail lines.
    - If stdout is not a TTY: prints only high-level log lines (no spam);
      subprocess output is kept in an in-memory tail buffer.
    """

    def __init__(self, *, tail_lines: int = 10, color: str = "auto") -> None:
        self.console = _console_for_color_mode(color)
        self._live_enabled = _is_interactive_default()

        self._tail = deque(maxlen=tail_lines)
        self._current_cmd: CommandInfo | None = None

        self._status: Mapping[str, str] | None = None

        self._overall = Progress(
            TextColumn("[bold]overall[/bold]"),
            BarColumn(bar_width=None),
            TaskProgressColumn(),
            TimeElapsedColumn(),
            console=self.console,
            transient=False,
        )
        self._overall_task_id = self._overall.add_task("steps", total=0)

        self._step = Progress(
            SpinnerColumn(),
            TextColumn("[bold]{task.description}[/bold]"),
            TimeElapsedColumn(),
            console=self.console,
            transient=False,
        )
        self._step_task_id = self._step.add_task("idle", total=None)

        self._live: Live | None = None
        self._last_refresh_s = 0.0

    @property
    def live_enabled(self) -> bool:
        return self._live_enabled

    def set_total_steps(self, total: int) -> None:
        self._overall.update(self._overall_task_id, total=total, completed=0)
        self._refresh()

    def set_status(self, values: Mapping[str, str]) -> None:
        self._status = values
        if not self._live_enabled:
            # Keep it single-line and grep-friendly.
            parts = " ".join(f"{k}={v}" for k, v in values.items())
            self.console.print(f"STATUS {parts}")
        self._refresh()

    def start(self) -> None:
        if not self._live_enabled:
            return
        # Keep refresh relatively low to avoid spamming terminals that don't handle
        # cursor rewrites well (e.g. some VS Code terminal configurations).
        self._live = Live(self._render(), console=self.console, refresh_per_second=4)
        self._live.start()

    def stop(self) -> None:
        if self._live is None:
            return

        total = self._overall.tasks[self._overall_task_id].total
        if total is not None:
            self._overall.update(self._overall_task_id, completed=total)
            self._refresh()

        self._live.stop()
        self._live = None

    def _refresh(self) -> None:
        if self._live is None:
            return
        now = time.monotonic()
        if (now - self._last_refresh_s) < 0.20:
            return
        self._last_refresh_s = now
        self._live.update(self._render())

    def tail(self, message: str, *, style: str | None = None) -> None:
        """Append to the rolling tail buffer. Does not print in non-TTY mode."""
        msg = Text(message)
        if style is not None:
            msg.stylize(style)
        self._tail.append(msg)
        self._refresh()

    def log(self, message: str, *, style: str | None = None) -> None:
        """High-level log line (prints in non-TTY, shows in tail in TTY)."""
        if self._live_enabled:
            self.tail(message, style=style)
            return

        text = Text(message)
        if style is not None:
            text.stylize(style)
        self.console.print(text)
        self._tail.append(text)

    def set_current_command(
        self,
        *,
        label: str,
        argv: Sequence[str],
        cwd: Path | None,
        env: Mapping[str, str] | None,
    ) -> None:
        self._current_cmd = CommandInfo(
            label=label,
            argv=[str(a) for a in argv],
            cwd=cwd,
            env=env,
        )
        if self._live_enabled:
            self._refresh()
        else:
            # Docker/CI-friendly: print command once when it changes.
            cmd = self._current_cmd
            argv = " ".join(cmd.argv)
            self.console.print(Text(f"cmd[{cmd.label}]: ", style="bold") + Text(argv))

    @contextmanager
    def step(self, title: str) -> Iterable[None]:
        if not self._live_enabled:
            self.console.print(Text("==> ", style="bold cyan") + Text(title))
            try:
                yield
            finally:
                self.console.print(Text("<== ", style="bold cyan") + Text(title))
            return

        self._step.update(self._step_task_id, description=title)
        self._refresh()
        try:
            yield
        finally:
            self._overall.advance(self._overall_task_id, 1)
            self._step.update(self._step_task_id, description="idle")
            self._refresh()

    def _render(self) -> Group:
        status = self._render_status()
        cmd = self._render_command()
        tail = self._render_tail()
        return Group(status, self._overall, self._step, cmd, tail)

    def _render_status(self) -> Text:
        if not self._status:
            return Text("STATUS -", style="dim")
        parts = " ".join(f"{k}={v}" for k, v in self._status.items())
        return Text(f"STATUS {parts}")

    def _render_command(self) -> Text:
        if self._current_cmd is None:
            return Text("CMD -", style="dim")

        cmd = self._current_cmd
        line = Text(f"CMD[{cmd.label}] ", style="bold") + Text(" ".join(cmd.argv))
        if cmd.cwd is not None:
            line.append(Text(f"  (cwd={cmd.cwd})", style="dim"))
        return line

    def _render_tail(self) -> Text:
        if not self._tail:
            return Text("TAIL (empty)", style="dim")

        body = Text("TAIL\n", style="bold")
        body.append(Text("\n").join(self._tail))
        return body
