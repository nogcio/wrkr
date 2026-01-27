"""
Server lifecycle management (wrkr-testserver).

Provides:
- TestServer: context manager for starting/stopping testserver.
- ServerTargets: parsed HTTP_URL and GRPC_URL.
"""

from __future__ import annotations

import contextlib
import subprocess
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path


class ServerError(RuntimeError):
    """Raised when server cannot start or provide targets."""


@dataclass(frozen=True, slots=True)
class ServerTargets:
    """Parsed server targets."""

    http_url: str
    grpc_url: str


class TestServer:
    """
    Manage wrkr-testserver lifecycle.

    Starts testserver, waits for HTTP_URL and GRPC_URL lines,
    then provides them to callers.
    """

    def __init__(self, *, proc: subprocess.Popen[bytes], started_at: float) -> None:
        self._proc = proc
        self._started_at = started_at
        self._http_url: str | None = None
        self._grpc_url: str | None = None

        self._stdout_thread: threading.Thread | None = None
        self._stderr_thread: threading.Thread | None = None

        self._stderr_tail_lock = threading.Lock()
        self._stderr_tail: list[str] = []

    @classmethod
    def start(
        cls, *, root: Path, server_bin: Path, on_log: Callable[[str], None] | None = None
    ) -> TestServer:
        """
        Start wrkr-testserver.

        Parameters
        ----------
        root : Path
            Repository root (process cwd).
        server_bin : Path
            Path to wrkr-testserver executable.

        Returns
        -------
        TestServer
        """
        root = root.resolve()
        server_bin = server_bin.resolve()

        if not server_bin.exists():
            raise ServerError(f"wrkr-testserver binary not found: {server_bin}")

        if on_log is None:
            print("Starting wrkr-testserver...", flush=True)
        else:
            on_log("Starting wrkr-testserver...")

        proc = subprocess.Popen(
            [str(server_bin), "--bind", "127.0.0.1:0"],
            cwd=str(root),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        srv = cls(proc=proc, started_at=time.monotonic())
        srv._start_reader_threads()
        return srv

    def wait_for_targets(
        self, *, timeout_s: float, on_log: Callable[[str], None] | None = None
    ) -> ServerTargets:
        """
        Wait for HTTP_URL and GRPC_URL from server.

        Parameters
        ----------
        timeout_s : float
            Timeout in seconds.

        Returns
        -------
        ServerTargets

        Raises
        ------
        ServerError
            If server exits early or timeout elapses.
        """
        if timeout_s <= 0:
            raise ValueError("timeout_s must be > 0")

        deadline = time.monotonic() + timeout_s
        while True:
            if self._http_url is not None and self._grpc_url is not None:
                if on_log is None:
                    print(f"HTTP: {self._http_url}", flush=True)
                    print(f"gRPC: {self._grpc_url}", flush=True)
                else:
                    on_log(f"HTTP: {self._http_url}")
                    on_log(f"gRPC: {self._grpc_url}")
                return ServerTargets(http_url=self._http_url, grpc_url=self._grpc_url)

            rc = self._proc.poll()
            if rc is not None:
                stderr = self._stderr_tail_text()
                raise ServerError(
                    "testserver exited early:\n"
                    f"  returncode: {rc}\n"
                    f"  elapsed_s: {self._elapsed_s():.3f}\n"
                    "--- stderr (tail) ---\n"
                    f"{stderr}\n"
                )

            if time.monotonic() > deadline:
                stderr = self._stderr_tail_text()
                raise ServerError(
                    "timed out waiting for HTTP_URL/GRPC_URL from testserver:\n"
                    f"  elapsed_s: {self._elapsed_s():.3f}\n"
                    "--- stderr (tail) ---\n"
                    f"{stderr}\n"
                )

            time.sleep(0.01)

    def shutdown(self) -> None:
        """Best-effort shutdown (safe to call multiple times)."""
        try:
            if self._proc.poll() is None:
                self._proc.kill()
        except OSError:
            pass

        with contextlib.suppress(Exception):
            self._proc.wait(timeout=2.0)

    def __enter__(self) -> TestServer:
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.shutdown()

    def _start_reader_threads(self) -> None:
        if self._proc.stdout is None:
            raise ServerError("testserver stdout pipe not available")
        if self._proc.stderr is None:
            raise ServerError("testserver stderr pipe not available")

        def read_stdout() -> None:
            assert self._proc.stdout is not None
            for raw in iter(self._proc.stdout.readline, b""):
                line = raw.decode("utf-8", errors="replace").rstrip("\r\n")
                if line.startswith("HTTP_URL="):
                    self._http_url = line.removeprefix("HTTP_URL=").strip()
                elif line.startswith("GRPC_URL="):
                    self._grpc_url = line.removeprefix("GRPC_URL=").strip()
                # Keep draining stdout even after we learn the targets.
                # If the server logs to stdout under load and we stop reading,
                # its stdout pipe can fill and block the server process.

        def read_stderr() -> None:
            assert self._proc.stderr is not None
            for raw in iter(self._proc.stderr.readline, b""):
                line = raw.decode("utf-8", errors="replace").rstrip("\r\n")
                if line == "":
                    continue
                self._append_stderr_tail(line)

        self._stdout_thread = threading.Thread(
            target=read_stdout, name="wrkr-testserver-stdout", daemon=True
        )
        self._stderr_thread = threading.Thread(
            target=read_stderr, name="wrkr-testserver-stderr", daemon=True
        )
        self._stdout_thread.start()
        self._stderr_thread.start()

    def _append_stderr_tail(self, line: str, *, max_lines: int = 200) -> None:
        with self._stderr_tail_lock:
            self._stderr_tail.append(line)
            if len(self._stderr_tail) > max_lines:
                self._stderr_tail = self._stderr_tail[-max_lines:]

    def _stderr_tail_text(self, *, tail_lines: int = 50) -> str:
        with self._stderr_tail_lock:
            if not self._stderr_tail:
                return ""
            return "\n".join(self._stderr_tail[-tail_lines:])

    def _elapsed_s(self) -> float:
        return time.monotonic() - self._started_at
