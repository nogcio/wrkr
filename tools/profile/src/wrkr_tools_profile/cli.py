from __future__ import annotations

import sys
from typing import Annotated

import typer

from .profile import ProfileConfig, ProfileError, run_profile

app = typer.Typer(
    add_completion=False,
    no_args_is_help=True,
    pretty_exceptions_enable=True,
    help="CPU profiling helper for wrkr.",
)


@app.command()
def grpc(
    sample_duration: Annotated[
        int,
        typer.Option(
            "--sample-duration",
            help="How long to sample stack traces (seconds).",
        ),
    ] = 10,
    load_duration: Annotated[
        str,
        typer.Option(
            "--load-duration",
            help="How long to keep the load test running (e.g. 30s).",
        ),
    ] = "30s",
    vus: Annotated[
        int,
        typer.Option(
            "--vus",
            help="Number of virtual users.",
            min=1,
        ),
    ] = 256,
    script: Annotated[
        str,
        typer.Option(
            "--script",
            help="Lua script to run (relative to repo root).",
        ),
    ] = "tools/perf/wrkr_grpc_plaintext.lua",
) -> None:
    """
    Run a gRPC profiling session (replaces profile_grpc_sample.sh).

    Builds wrkr in profiling mode, starts testserver, warms up, then runs wrkr
    under macOS 'sample' to capture CPU stack traces.
    """
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=0,
    )
    try:
        run_profile(cfg)
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None


@app.command()
def wfb_grpc(
    sample_duration: Annotated[
        int,
        typer.Option(
            "--sample-duration",
            help="How long to sample stack traces (seconds).",
        ),
    ] = 10,
    load_duration: Annotated[
        str,
        typer.Option(
            "--load-duration",
            help="How long to keep the load test running (e.g. 30s).",
        ),
    ] = "30s",
    vus: Annotated[
        int,
        typer.Option(
            "--vus",
            help="Number of virtual users.",
            min=1,
        ),
    ] = 50,
    pre_sample_sleep: Annotated[
        int,
        typer.Option(
            "--pre-sample-sleep",
            help="Delay before starting sampling (helps avoid startup skew).",
        ),
    ] = 5,
    script: Annotated[
        str,
        typer.Option(
            "--script",
            help="Lua script to run (relative to repo root).",
        ),
    ] = "tools/perf/wfb_grpc_aggregate.lua",
) -> None:
    """
    Run a wfb gRPC profiling session (replaces profile_wfb_grpc_aggregate_sample.sh).

    Builds wrkr in profiling mode, starts testserver, warms up, then runs wrkr
    under macOS 'sample' to capture CPU stack traces (with pre-sample sleep to
    avoid proto compilation overhead).
    """
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=pre_sample_sleep,
    )
    try:
        run_profile(cfg)
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None
