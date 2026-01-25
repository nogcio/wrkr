from __future__ import annotations

import os
from pathlib import Path
from typing import Annotated

import typer

from .app import config_from_values
from .app import run as run_suite
from .config import ConfigError, env_path

app = typer.Typer(
    add_completion=False,
    no_args_is_help=True,
    pretty_exceptions_enable=True,
    help="Cross-platform perf comparison runner for wrkr/wrk/k6 (Python + uv).",
)


def _env_str(name: str) -> str | None:
    v = os.environ.get(name)
    if v is None:
        return None
    s = v.strip()
    return s if s else None


def _env_int(name: str) -> int | None:
    v = _env_str(name)
    if v is None:
        return None
    try:
        return int(v)
    except ValueError as e:
        raise ConfigError(f"Invalid integer in env {name}={v!r}") from e


def _env_float(name: str) -> float | None:
    v = _env_str(name)
    if v is None:
        return None
    try:
        return float(v)
    except ValueError as e:
        raise ConfigError(f"Invalid float in env {name}={v!r}") from e


def _default_root() -> Path:
    # Match prior behavior: --root defaults to current working directory.
    return Path.cwd()


def _resolve_root(root: Path | None) -> Path:
    # Also support WRKR_ROOT (matches previous behavior).
    if root is not None:
        return root
    env_root = env_path("WRKR_ROOT")
    return env_root if env_root is not None else _default_root()


@app.command()
def run(
    # Root / duration / build flags
    root: Annotated[
        Path | None,
        typer.Option(
            "--root",
            help="Root of the wrkr repository (defaults to current working directory).",
            envvar="WRKR_ROOT",
            dir_okay=True,
            file_okay=False,
            readable=True,
        ),
    ] = None,
    duration: Annotated[
        str,
        typer.Option(
            "--duration",
            help="Duration to run each case (e.g. 5s).",
            envvar="DURATION",
        ),
    ] = "5s",
    build: Annotated[
        bool,
        typer.Option(
            "--build/--no-build",
            help="Build required binaries before running.",
        ),
    ] = True,
    native: Annotated[
        bool,
        typer.Option(
            "--native/--no-native",
            help="Build with -C target-cpu=native (best perf, machine-specific).",
            envvar="NATIVE",
        ),
    ] = True,
    # Load generator tuning
    wrkr_vus: Annotated[
        int,
        typer.Option(
            "--wrkr-vus",
            help="Number of virtual users for wrkr.",
            envvar="WRKR_VUS",
            min=1,
        ),
    ] = 256,
    k6_vus: Annotated[
        int | None,
        typer.Option(
            "--k6-vus",
            help="Number of VUs for k6 (defaults to wrkr_vus).",
            envvar="K6_VUS",
            min=1,
        ),
    ] = None,
    wrk_threads: Annotated[
        int,
        typer.Option(
            "--wrk-threads",
            help="wrk threads.",
            envvar="WRK_THREADS",
            min=1,
        ),
    ] = 8,
    wrk_connections: Annotated[
        int,
        typer.Option(
            "--wrk-connections",
            help="wrk connections.",
            envvar="WRK_CONNECTIONS",
            min=1,
        ),
    ] = 256,
    # Gates / ratios
    ratio_ok_get_hello: Annotated[
        float,
        typer.Option(
            "--ratio-ok-get-hello",
            help="Gate: wrkr_rps must be >= wrk_rps * ratio.",
            envvar="RATIO_OK",
        ),
    ] = 0.90,
    ratio_ok_post_json: Annotated[
        float,
        typer.Option(
            "--ratio-ok-post-json",
            help="Gate: wrkr_rps must be >= wrk_rps * ratio.",
            envvar="RATIO_OK_POST_JSON",
        ),
    ] = 0.90,
    ratio_ok_wfb_json_aggregate: Annotated[
        float,
        typer.Option(
            "--ratio-ok-wfb-json-aggregate",
            help="Gate: wrkr_rps must be >= wrk_rps * ratio.",
            envvar="RATIO_OK_WFB_JSON_AGGREGATE",
        ),
    ] = 0.90,
    ratio_ok_wrkr_over_k6: Annotated[
        float,
        typer.Option(
            "--ratio-ok-wrkr-over-k6",
            help="Gate: wrkr_rps must be > k6_rps * ratio.",
            envvar="RATIO_OK_WRKR_OVER_K6",
        ),
    ] = 1.40,
    ratio_ok_grpc_wrkr_over_k6: Annotated[
        float,
        typer.Option(
            "--ratio-ok-grpc-wrkr-over-k6",
            help="Gate for gRPC: wrkr_rps must be > k6_rps * ratio.",
            envvar="RATIO_OK_GRPC_WRKR_OVER_K6",
        ),
    ] = 2.00,
    ratio_ok_wfb_grpc_aggregate_wrkr_over_k6: Annotated[
        float,
        typer.Option(
            "--ratio-ok-wfb-grpc-aggregate-wrkr-over-k6",
            help="Gate for wfb gRPC AggregateOrders: wrkr_rps must be > k6_rps * ratio.",
            envvar="RATIO_OK_WFB_GRPC_AGGREGATE_WRKR_OVER_K6",
        ),
    ] = 1.20,
    ratio_ok_grpc_wrkr_over_wrk_hello: Annotated[
        float,
        typer.Option(
            "--ratio-ok-grpc-wrkr-over-wrk-hello",
            help="Optional cross-protocol gate: wrkr gRPC RPS must be >= wrk GET /hello RPS * ratio.",
            envvar="RATIO_OK_GRPC_WRKR_OVER_WRK_HELLO",
        ),
    ] = 0.70,
    # Tool requirements
    require_wrk: Annotated[
        bool,
        typer.Option(
            "--require-wrk",
            help="If set, missing wrk is a hard error; otherwise HTTP wrk comparisons are skipped.",
        ),
    ] = False,
    require_k6: Annotated[
        bool,
        typer.Option(
            "--require-k6",
            help="If set, missing k6 is a hard error; otherwise k6 comparisons are skipped.",
        ),
    ] = False,
    color: Annotated[
        str,
        typer.Option(
            "--color",
            help="Color output mode for final summaries (auto|always|never). Use always when piping to tail.",
            envvar="WRKR_TOOLS_COMPARE_PERF_COLOR",
            show_default=True,
        ),
    ] = "auto",
) -> None:
    """
    Run the full perf comparison suite.

    This command is the primary interface and is intended to fully replace
    the former Rust tool's CLI surface.
    """
    # Also accept the exact legacy env var names for a couple of flags where clap used env=...
    # Typer's envvar already covers these, but we keep a tiny bit of compatibility logic for
    # users who rely on env-only overrides while invoking without flags.
    if "RATIO_OK" in os.environ and ratio_ok_get_hello == 0.90:
        v = _env_float("RATIO_OK")
        if v is not None:
            ratio_ok_get_hello = v

    root_path = _resolve_root(root)

    color_norm = color.strip().lower()
    if color_norm not in {"auto", "always", "never"}:
        raise ConfigError(
            f"Invalid --color value: {color!r} (expected one of: auto, always, never)"
        )
    color = color_norm

    cfg = config_from_values(
        root=root_path,
        duration=duration,
        build=build,
        native=native,
        wrkr_vus=wrkr_vus,
        k6_vus=k6_vus,
        wrk_threads=wrk_threads,
        wrk_connections=wrk_connections,
        ratio_ok_get_hello=ratio_ok_get_hello,
        ratio_ok_post_json=ratio_ok_post_json,
        ratio_ok_wfb_json_aggregate=ratio_ok_wfb_json_aggregate,
        ratio_ok_wrkr_over_k6=ratio_ok_wrkr_over_k6,
        ratio_ok_grpc_wrkr_over_k6=ratio_ok_grpc_wrkr_over_k6,
        ratio_ok_wfb_grpc_aggregate_wrkr_over_k6=ratio_ok_wfb_grpc_aggregate_wrkr_over_k6,
        ratio_ok_grpc_wrkr_over_wrk_hello=ratio_ok_grpc_wrkr_over_wrk_hello,
        require_wrk=require_wrk,
        require_k6=require_k6,
    )

    try:
        outcome = run_suite(cfg, color=color)
    except ConfigError as e:
        typer.secho(f"CONFIG ERROR: {e}", fg=typer.colors.RED, err=True)
        raise typer.Exit(code=2) from e
    except Exception as e:
        # Keep as a clear non-zero exit, but preserve tracebacks when TYper decides to show them.
        typer.secho(f"ERROR: {e}", fg=typer.colors.RED, err=True)
        raise typer.Exit(code=1) from e

    if not outcome.ok():
        raise typer.Exit(code=1)


@app.callback(invoke_without_command=True)
def _default(
    ctx: typer.Context,
) -> None:
    """
    Default behavior: run the suite (matching prior tool which ran immediately).
    """
    if ctx.invoked_subcommand is None:
        # Delegate to the default command (this keeps UX close to the previous tool).
        ctx.invoke(run)


def main() -> None:
    """
    Programmatic entrypoint used by `project.scripts`.
    """
    # Typer uses Click under the hood; ensure consistent behavior for non-interactive CI logs.
    # `standalone_mode=True` makes Click handle SystemExit for us.
    app(prog_name="wrkr-tools-compare-perf")


if __name__ == "__main__":
    main()
