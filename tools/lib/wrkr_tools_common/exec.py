"""
Subprocess execution utilities with RSS sampling.

Provides:
- run_with_peak_rss_sampling: Execute subprocess and measure peak memory.
- run_with_peak_rss_sampling_streaming: Same but streams output lines via callbacks.
- run_checked: Execute subprocess or raise on non-zero exit.
- run_checked_streaming: Same but streams output lines via callbacks.
- print_invocation: Format and log command execution.
"""

from __future__ import annotations

import os
import shlex
import subprocess
import sys
import threading
import time
from collections.abc import Iterable, Mapping, Sequence
from contextlib import suppress
from dataclasses import dataclass
from pathlib import Path

_DEFAULT_CHUNK_SIZE = 16 * 1024


class ExecError(RuntimeError):
    """Raised when a subprocess fails."""


@dataclass(frozen=True, slots=True)
class RunResult:
    """Result of a subprocess execution."""

    returncode: int
    stdout: str
    stderr: str
    peak_rss_bytes: int


def quote_for_display(s: str) -> str:
    """Quote string for display (not shell-safe, for logs only)."""
    if not any(c.isspace() or c in {'"', "\\"} for c in s):
        return s
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def format_command(argv: Sequence[str | os.PathLike[str]]) -> str:
    """Format argv as readable one-liner."""
    parts: list[str] = []
    for a in argv:
        s = os.fspath(a)
        parts.append(quote_for_display(s))
    return " ".join(parts)


def print_invocation(
    *,
    label: str,
    argv: Sequence[str | os.PathLike[str]],
    cwd: Path | None = None,
    extra_env: Mapping[str, str] | None = None,
) -> None:
    """Print command invocation details."""
    if cwd is not None:
        print(f"{label}: cwd={cwd}", flush=True)
    if extra_env:
        pairs = " ".join(f"{k}={quote_for_display(v)}" for k, v in extra_env.items())
        print(f"{label}: env {pairs}", flush=True)
    print(f"{label}: {format_command(argv)}", flush=True)


def _read_rss_bytes_best_effort(pid: int) -> int | None:
    """Best-effort RSS reader (macOS/Linux)."""
    if pid <= 0:
        return None

    if sys.platform == "linux":
        statm = Path(f"/proc/{pid}/statm")
        try:
            txt = statm.read_text(encoding="utf-8")
            parts = txt.split()
            if len(parts) < 2:
                return None
            resident_pages = int(parts[1])
            page_size = os.sysconf("SC_PAGE_SIZE")
            return resident_pages * int(page_size)
        except (FileNotFoundError, ProcessLookupError, PermissionError, ValueError, OSError):
            return None

    if sys.platform == "darwin":
        try:
            cp = subprocess.run(
                ["ps", "-o", "rss=", "-p", str(pid)],
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                text=True,
            )
            if cp.returncode != 0:
                return None
            v = cp.stdout.strip()
            if not v:
                return None
            kib = int(v.split()[0])
            return kib * 1024
        except (ValueError, OSError):
            return None

    return None


def run_with_peak_rss_sampling(
    argv: Sequence[str | os.PathLike[str]],
    *,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    stdin: int | None = subprocess.DEVNULL,
    sample_interval_s: float = 0.05,
) -> RunResult:
    """
    Execute subprocess with stdout/stderr capture and peak RSS sampling.

    Parameters
    ----------
    argv : Sequence
        Command arguments (no shell).
    cwd : Path, optional
        Working directory.
    env : Mapping, optional
        Extra environment variables (merged with os.environ).
    stdin : int, optional
        stdin file descriptor (default DEVNULL).
    sample_interval_s : float
        RSS sampling interval in seconds.

    Returns
    -------
    RunResult
        returncode, stdout, stderr, peak_rss_bytes.
    """
    if not argv:
        raise ValueError("argv must be non-empty")

    if sample_interval_s <= 0:
        raise ValueError("sample_interval_s must be > 0")

    full_env: dict[str, str] | None
    if env is None:
        full_env = None
    else:
        full_env = dict(os.environ)
        full_env.update({k: str(v) for k, v in env.items()})

    proc = subprocess.Popen(
        [os.fspath(a) for a in argv],
        cwd=os.fspath(cwd) if cwd is not None else None,
        env=full_env,
        stdin=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    peak_lock = threading.Lock()
    peak_rss: int = 0
    stop = threading.Event()

    def sampler() -> None:
        nonlocal peak_rss
        while not stop.is_set():
            rss = _read_rss_bytes_best_effort(proc.pid)
            if rss is not None:
                with peak_lock:
                    if rss > peak_rss:
                        peak_rss = rss
            time.sleep(sample_interval_s)

    t = threading.Thread(target=sampler, name="rss-sampler", daemon=True)
    t.start()

    out_b, err_b = proc.communicate()
    stop.set()
    t.join(timeout=1.0)

    stdout = out_b.decode("utf-8", errors="replace") if out_b is not None else ""
    stderr = err_b.decode("utf-8", errors="replace") if err_b is not None else ""

    with peak_lock:
        peak = peak_rss

    return RunResult(
        returncode=int(proc.returncode or 0),
        stdout=stdout,
        stderr=stderr,
        peak_rss_bytes=int(peak),
    )


def run_with_peak_rss_sampling_streaming(
    argv: Sequence[str | os.PathLike[str]],
    *,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    stdin: int | None = subprocess.DEVNULL,
    sample_interval_s: float = 0.05,
    on_stdout_line=None,
    on_stderr_line=None,
) -> RunResult:
    """Execute subprocess while streaming output via callbacks.

    This is a sibling of `run_with_peak_rss_sampling` that reads stdout/stderr
    concurrently, calling `on_stdout_line(line)` / `on_stderr_line(line)` for
    each parsed line-like chunk (splits on `\n` and `\r`).

    Output is still fully captured and returned in the `RunResult`.
    """
    if not argv:
        raise ValueError("argv must be non-empty")

    if sample_interval_s <= 0:
        raise ValueError("sample_interval_s must be > 0")

    full_env: dict[str, str] | None
    if env is None:
        full_env = None
    else:
        full_env = dict(os.environ)
        full_env.update({k: str(v) for k, v in env.items()})

    proc = subprocess.Popen(
        [os.fspath(a) for a in argv],
        cwd=os.fspath(cwd) if cwd is not None else None,
        env=full_env,
        stdin=stdin,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=0,
    )

    peak_lock = threading.Lock()
    peak_rss: int = 0
    stop = threading.Event()

    def sampler() -> None:
        nonlocal peak_rss
        while not stop.is_set():
            rss = _read_rss_bytes_best_effort(proc.pid)
            if rss is not None:
                with peak_lock:
                    if rss > peak_rss:
                        peak_rss = rss
            time.sleep(sample_interval_s)

    out_chunks: list[str] = []
    err_chunks: list[str] = []

    def _iter_lines_from_bytes(chunks: Iterable[bytes]) -> Iterable[str]:
        # Split on both \n and \r so tools like k6 that redraw a single line still show updates.
        buf = ""
        for b in chunks:
            buf += b.decode("utf-8", errors="replace")
            while True:
                idx_n = buf.find("\n")
                idx_r = buf.find("\r")
                idxs = [i for i in (idx_n, idx_r) if i != -1]
                if not idxs:
                    break
                i = min(idxs)
                line = buf[:i]
                buf = buf[i + 1 :]
                if line:
                    yield line
        if buf:
            yield buf

    def _reader(pipe, *, sink: list[str], cb) -> None:
        if pipe is None:
            return
        try:

            def chunk_iter() -> Iterable[bytes]:
                while True:
                    b = pipe.read(_DEFAULT_CHUNK_SIZE)
                    if not b:
                        break
                    yield b

            for line in _iter_lines_from_bytes(chunk_iter()):
                sink.append(line + "\n")
                if cb is not None:
                    with suppress(Exception):
                        cb(line)
        finally:
            with suppress(Exception):
                pipe.close()

    t_sampler = threading.Thread(target=sampler, name="rss-sampler", daemon=True)
    t_out = threading.Thread(
        target=_reader,
        args=(proc.stdout,),
        kwargs={"sink": out_chunks, "cb": on_stdout_line},
        daemon=True,
    )
    t_err = threading.Thread(
        target=_reader,
        args=(proc.stderr,),
        kwargs={"sink": err_chunks, "cb": on_stderr_line},
        daemon=True,
    )

    t_sampler.start()
    t_out.start()
    t_err.start()

    returncode = proc.wait()
    stop.set()
    t_out.join(timeout=2.0)
    t_err.join(timeout=2.0)
    t_sampler.join(timeout=1.0)

    with peak_lock:
        peak = peak_rss

    return RunResult(
        returncode=int(returncode or 0),
        stdout="".join(out_chunks),
        stderr="".join(err_chunks),
        peak_rss_bytes=int(peak),
    )


def run_checked(
    argv: Sequence[str | os.PathLike[str]],
    *,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    stdin: int | None = subprocess.DEVNULL,
    label: str | None = None,
) -> RunResult:
    """
    Execute subprocess or raise on non-zero exit.

    Parameters
    ----------
    argv : Sequence
        Command arguments.
    cwd : Path, optional
        Working directory.
    env : Mapping, optional
        Extra environment variables.
    stdin : int, optional
        stdin file descriptor.
    label : str, optional
        Log label for print_invocation.

    Returns
    -------
    RunResult

    Raises
    ------
    ExecError
        If command returns non-zero.
    """
    if label is not None:
        print_invocation(label=label, argv=argv, cwd=cwd, extra_env=env)

    res = run_with_peak_rss_sampling(argv, cwd=cwd, env=env, stdin=stdin)
    if res.returncode != 0:
        cmd = " ".join(shlex.quote(os.fspath(a)) for a in argv)
        tail_out = _tail_lines(res.stdout, 20)
        tail_err = _tail_lines(res.stderr, 20)
        raise ExecError(
            "Command failed:\n"
            f"  cmd: {cmd}\n"
            f"  returncode: {res.returncode}\n"
            "--- stdout (tail) ---\n"
            f"{tail_out}\n"
            "--- stderr (tail) ---\n"
            f"{tail_err}\n"
        )
    return res


def run_checked_streaming(
    argv: Sequence[str | os.PathLike[str]],
    *,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
    stdin: int | None = subprocess.DEVNULL,
    label: str | None = None,
    on_stdout_line=None,
    on_stderr_line=None,
) -> RunResult:
    """Streaming variant of `run_checked` (see `run_with_peak_rss_sampling_streaming`)."""
    if label is not None:
        print_invocation(label=label, argv=argv, cwd=cwd, extra_env=env)

    res = run_with_peak_rss_sampling_streaming(
        argv,
        cwd=cwd,
        env=env,
        stdin=stdin,
        on_stdout_line=on_stdout_line,
        on_stderr_line=on_stderr_line,
    )

    if res.returncode != 0:
        cmd = " ".join(shlex.quote(os.fspath(a)) for a in argv)
        tail_out = _tail_lines(res.stdout, 20)
        tail_err = _tail_lines(res.stderr, 20)
        raise ExecError(
            "Command failed:\n"
            f"  cmd: {cmd}\n"
            f"  returncode: {res.returncode}\n"
            "--- stdout (tail) ---\n"
            f"{tail_out}\n"
            "--- stderr (tail) ---\n"
            f"{tail_err}\n"
        )
    return res


def _tail_lines(text: str, n: int) -> str:
    """Return last n lines of text."""
    if n <= 0:
        return ""
    lines = text.splitlines()
    if len(lines) <= n:
        return text
    return "\n".join(lines[-n:])
