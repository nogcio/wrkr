from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

from rich.text import Text

from .build import BuildPlan, build_binaries
from .cases import default_grpc_cases, default_http_cases, is_too_slow, run_grpc_case, run_http_case
from .config import (
    Config,
    ConfigError,
    Ratios,
    RunTuning,
    ToolRequirements,
    validate_ratios,
    validate_tuning,
)
from .parse import Rps
from .server import TestServer
from .tool_detection import detect_tools
from .ui import RunUI


class AppError(RuntimeError):
    """Raised when the overall app execution fails."""


@dataclass(frozen=True, slots=True)
class OverallOutcome:
    failures: int

    def ok(self) -> bool:
        return self.failures == 0


def run(cfg: Config, *, color: str = "auto") -> OverallOutcome:
    """
    Orchestrate a full perf comparison run.

    This coordinates:
      - config validation
      - optional builds
      - tool detection (wrk/k6 optional)
      - starting wrkr-testserver and acquiring targets
      - running all default HTTP and gRPC cases
      - the cross-protocol gate (wrkr gRPC vs wrk hello), when possible

    The caller (CLI) is responsible for translating failures into exit codes.
    """
    _validate_config(cfg)

    ui = RunUI(color=color)
    ui.start()
    stopped = False
    try:
        if cfg.tuning.build:
            build_binaries(BuildPlan(root=cfg.root, native=cfg.tuning.native), ui=ui)

        with ui.step("detect tools"):
            tools = detect_tools(cfg.root, cfg.requirements)

        failures = 0
        failure_summary: list[str] = []
        case_summaries: list[tuple[str, ...]] = []

        ui.set_status(
            {
                "duration": cfg.tuning.duration,
                "wrk": f"threads={cfg.tuning.wrk_threads} conns={cfg.tuning.wrk_connections}",
                "wrkr": f"vus={cfg.tuning.wrkr_vus}",
                "k6": f"vus={cfg.effective_k6_vus()}",
            }
        )

        http_cases = default_http_cases(cfg)
        grpc_cases = default_grpc_cases(cfg)

        # Once we know which tools are present, compute a stable total step count so
        # progress bars can be meaningful even in Docker/CI logs.
        steps = 0
        if cfg.tuning.build:
            steps += 2  # cargo build wrkr-testserver + wrkr
        steps += 1  # detect tools
        steps += 1  # wait for testserver
        steps += 1  # cross-protocol gate

        for _ in http_cases:
            if tools.wrk is not None:
                steps += 1
            steps += 1  # wrkr
            if tools.k6 is not None:
                steps += 1
        for _ in grpc_cases:
            steps += 1  # wrkr
            if tools.k6 is not None:
                steps += 1

        ui.set_total_steps(steps)

        hello_wrk_rps: Rps | None = None
        grpc_first_wrkr_rps: Rps | None = None

        targets_http_url: str | None = None
        targets_grpc_url: str | None = None

        server = TestServer.start(
            root=cfg.root,
            server_bin=tools.wrkr_testserver,
            on_log=lambda m: ui.tail(m, style="dim"),
        )
        with server:
            with ui.step("wait for testserver"):
                targets = server.wait_for_targets(
                    timeout_s=5.0,
                    on_log=lambda m: ui.tail(m, style="dim"),
                )

            targets_http_url = targets.http_url
            targets_grpc_url = targets.grpc_url

            ui.set_status({"http_url": targets.http_url, "grpc_url": targets.grpc_url})

            # HTTP cases
            for i, case in enumerate(http_cases):
                outcome = run_http_case(
                    cfg=cfg,
                    tools=tools,
                    base_url=targets.http_url,
                    case=case,
                    ui=ui,
                )
                failures += outcome.failures
                for msg in outcome.failure_messages:
                    failure_summary.append(f"HTTP {case.title}: {msg}")
                case_summaries.append(outcome.summary_lines)

                # Keep wrk RPS for hello to power the cross-protocol gate.
                if i == 0:
                    hello_wrk_rps = outcome.wrk_rps

            # gRPC cases
            for i, case in enumerate(grpc_cases):
                outcome = run_grpc_case(
                    cfg=cfg,
                    tools=tools,
                    grpc_url=targets.grpc_url,
                    case=case,
                    ui=ui,
                )
                failures += outcome.failures
                for msg in outcome.failure_messages:
                    failure_summary.append(f"gRPC {case.title}: {msg}")
                case_summaries.append(outcome.summary_lines)

                # Use the first gRPC case (Echo plaintext) for cross-protocol gate.
                if i == 0:
                    grpc_first_wrkr_rps = outcome.wrkr_rps

        # Cross-protocol comparison: wrkr gRPC vs wrk GET /hello.
        with ui.step("cross-protocol gate"):
            gate_failure = _cross_protocol_gate(
                cfg=cfg,
                grpc_wrkr_rps=grpc_first_wrkr_rps,
                wrk_hello_rps=hello_wrk_rps,
                ui=ui,
            )
            if gate_failure is not None:
                failures += 1
                failure_summary.append(f"cross-protocol: {gate_failure}")

        if failures > 0:
            ui.log(f"OVERALL: FAIL ({failures} failing case(s))")
        else:
            ui.log("OVERALL: PASS")

        # Print a stable summary after Live stops so it's always visible even with a
        # short tail buffer.
        ui.stop()
        stopped = True

        console = ui.console

        console.print(Text("CONDITIONS:", style="bold cyan"))
        console.print(f"- duration={cfg.tuning.duration}")
        console.print(
            "- "
            f"wrkr_vus={cfg.tuning.wrkr_vus} "
            f"k6_vus={cfg.effective_k6_vus()} "
            f"wrk_threads={cfg.tuning.wrk_threads} "
            f"wrk_connections={cfg.tuning.wrk_connections}"
        )
        console.print(
            f"- targets: http_url={targets_http_url or '-'} grpc_url={targets_grpc_url or '-'}"
        )
        console.print(Text(f"- tool[wrkr]={tools.wrkr}"))
        console.print(Text(f"- tool[wrkr-testserver]={tools.wrkr_testserver}"))
        console.print(Text(f"- tool[wrk]={'-' if tools.wrk is None else tools.wrk}"))
        console.print(Text(f"- tool[k6]={'-' if tools.k6 is None else tools.k6}"))
        console.print(
            "- order: HTTP runs wrk -> wrkr -> k6; gRPC runs wrkr -> k6 (single shared testserver)"
        )

        if cfg.tuning.wrkr_vus != cfg.effective_k6_vus():
            console.print(
                Text(
                    "- WARNING: wrkr_vus != k6_vus; comparisons are not under equal VU counts",
                    style="bold yellow",
                )
            )
        if cfg.tuning.wrk_connections != cfg.tuning.wrkr_vus:
            console.print(
                Text(
                    "- WARNING: wrk_connections != wrkr_vus; wrk load intensity differs from wrkr VUs",
                    style="bold yellow",
                )
            )

        if case_summaries:
            console.print(Text("SUMMARY:", style="bold green"))
            for block in case_summaries:
                for line in block:
                    console.print(_style_summary_line(line))
                console.print("")

        if failure_summary:
            console.print(Text("FAILED:", style="bold red"))
            for line in failure_summary:
                console.print(Text(f"- {line}", style="red"))

        return OverallOutcome(failures=failures)
    finally:
        if not stopped:
            ui.stop()


def _validate_config(cfg: Config) -> None:
    if not isinstance(cfg.root, Path):
        raise ConfigError("Config.root must be a pathlib.Path")

    if not cfg.root.exists():
        raise ConfigError(f"wrkr root does not exist: {cfg.root}")

    validate_tuning(cfg.tuning)
    validate_ratios(cfg.ratios)


def _cross_protocol_gate(
    *,
    cfg: Config,
    grpc_wrkr_rps: Rps | None,
    wrk_hello_rps: Rps | None,
    ui: RunUI,
) -> str | None:
    """
    Optional gate: wrkr gRPC RPS must be >= wrk GET /hello RPS * ratio.

    This is only applied when:
      - wrk is installed (so hello wrk RPS exists)
      - we have a gRPC wrkr RPS from the first gRPC case
    """
    if wrk_hello_rps is None:
        msg = "INFO: wrkr-grpc/wrk-hello ratio skipped (wrk not installed or hello wrk skipped)"
        ui.log(msg)
        return None

    if grpc_wrkr_rps is None:
        msg = "INFO: wrkr-grpc/wrk-hello ratio skipped (gRPC case did not produce wrkr RPS)"
        ui.log(msg)
        return None

    ratio_ok = cfg.ratios.ratio_ok_grpc_wrkr_over_wrk_hello
    ratio_actual = (
        grpc_wrkr_rps.value / wrk_hello_rps.value if wrk_hello_rps.value > 0 else float("inf")
    )

    if is_too_slow(wrkr=grpc_wrkr_rps, other=wrk_hello_rps, ratio=ratio_ok, inclusive=True):
        msg = (
            "FAIL: wrkr grpc is too slow vs wrk hello "
            f"(ratio_ok={ratio_ok}, ratio_actual={ratio_actual:.3f})"
        )
        ui.log(msg)
        return msg

    msg = f"PASS: wrkr-grpc/wrk-hello >= {ratio_ok} (ratio_actual={ratio_actual:.3f})"
    ui.log(msg)
    return None


def _style_summary_line(line: str) -> Text:
    # Make summary blocks scannable in CI logs.
    t = Text(line)

    if line.startswith("HTTP ") or line.startswith("gRPC "):
        t.stylize("bold")
        return t

    if " scripts:" in line:
        t.stylize("dim")
        return t

    # Color OK/FAIL/SKIP.
    m = re.search(r"\b(OK|FAIL|SKIP)\b", line)
    if m is not None:
        word = m.group(1)
        style = {"OK": "green", "FAIL": "red", "SKIP": "yellow"}[word]
        t.stylize(style, m.start(1), m.end(1))

    # Color rps values.
    idx = 0
    while True:
        i = line.find("rps=", idx)
        if i == -1:
            break
        j = i + 4
        while j < len(line) and (line[j].isdigit() or line[j] == "." or line[j] == "-"):
            j += 1
        t.stylize("cyan", i + 4, j)
        idx = j

    # Color ratio_actual in gate lines based on threshold.
    if "ratio_ok=" in line and "ratio_actual=" in line:
        try:
            ratio_ok_s = line.split("ratio_ok=", 1)[1].split()[0]
            ratio_actual_s = line.split("ratio_actual=", 1)[1].split()[0]
            ratio_ok = float(ratio_ok_s)
            ratio_actual = float(ratio_actual_s)
            ra_pos = line.find("ratio_actual=")
            if ra_pos != -1:
                ra_val_start = ra_pos + len("ratio_actual=")
                ra_val_end = ra_val_start + len(ratio_actual_s)
                t.stylize("green" if ratio_actual >= ratio_ok else "red", ra_val_start, ra_val_end)
        except Exception:
            pass

    return t


def config_from_values(
    *,
    root: Path,
    duration: str = "5s",
    build: bool = True,
    native: bool = True,
    wrkr_vus: int = 256,
    k6_vus: int | None = None,
    wrk_threads: int = 8,
    wrk_connections: int = 256,
    # ratios
    ratio_ok_get_hello: float = 0.90,
    ratio_ok_post_json: float = 0.90,
    ratio_ok_wfb_json_aggregate: float = 0.90,
    ratio_ok_wrkr_over_k6: float = 1.40,
    ratio_ok_grpc_wrkr_over_k6: float = 2.00,
    ratio_ok_wfb_grpc_aggregate_wrkr_over_k6: float = 1.20,
    ratio_ok_grpc_wrkr_over_wrk_hello: float = 0.70,
    # tool requirements
    require_wrk: bool = False,
    require_k6: bool = False,
) -> Config:
    """
    Convenience constructor used by the CLI.

    Keeps the CLI file small by centralizing the config wiring here.
    """
    return Config(
        root=root,
        tuning=RunTuning(
            duration=duration,
            wrkr_vus=wrkr_vus,
            k6_vus=k6_vus,
            wrk_threads=wrk_threads,
            wrk_connections=wrk_connections,
            build=build,
            native=native,
        ),
        ratios=Ratios(
            ratio_ok_get_hello=ratio_ok_get_hello,
            ratio_ok_post_json=ratio_ok_post_json,
            ratio_ok_wfb_json_aggregate=ratio_ok_wfb_json_aggregate,
            ratio_ok_wrkr_over_k6=ratio_ok_wrkr_over_k6,
            ratio_ok_grpc_wrkr_over_k6=ratio_ok_grpc_wrkr_over_k6,
            ratio_ok_wfb_grpc_aggregate_wrkr_over_k6=ratio_ok_wfb_grpc_aggregate_wrkr_over_k6,
            ratio_ok_grpc_wrkr_over_wrk_hello=ratio_ok_grpc_wrkr_over_wrk_hello,
        ),
        requirements=ToolRequirements(require_wrk=require_wrk, require_k6=require_k6),
    )
