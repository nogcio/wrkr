from __future__ import annotations

import sys
from typing import Annotated

import typer

from .errors import ProfileError
from .profile import ProfileConfig, run_profile
from .samply_profile import SamplyConfig, run_samply_profile

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
    under a platform profiler to capture CPU stack traces:
    - macOS: 'sample'
    - Linux: 'perf'
    """
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=0,
        env_templates=("GRPC_TARGET={GRPC_TARGET}",),
    )
    try:
        run_profile(cfg)
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None


@app.command()
def grpc_samply(
    sample_duration: Annotated[
        int,
        typer.Option(
            "--sample-duration",
            help="How long samply records stacks (seconds).",
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
    open_ui: Annotated[
        bool,
        typer.Option(
            "--open/--no-open",
            help="Open the samply UI after recording.",
        ),
    ] = True,
    script: Annotated[
        str,
        typer.Option(
            "--script",
            help="Lua script to run (relative to repo root).",
        ),
    ] = "tools/perf/wrkr_grpc_plaintext.lua",
) -> None:
    """Run a gRPC profiling session using samply (Linux) and open the UI."""
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=0,
        env_templates=("GRPC_TARGET={GRPC_TARGET}",),
    )

    try:
        run_samply_profile(cfg, samply=SamplyConfig(open_ui=open_ui))
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
    under a platform profiler to capture CPU stack traces (with pre-sample sleep
    to avoid proto compilation overhead).
    """
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=pre_sample_sleep,
        env_templates=("GRPC_TARGET={GRPC_TARGET}",),
    )
    try:
        run_profile(cfg)
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None


@app.command()
def wfb_grpc_samply(
    sample_duration: Annotated[
        int,
        typer.Option(
            "--sample-duration",
            help="How long samply records stacks (seconds).",
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
    open_ui: Annotated[
        bool,
        typer.Option(
            "--open/--no-open",
            help="Open the samply UI after recording.",
        ),
    ] = True,
    script: Annotated[
        str,
        typer.Option(
            "--script",
            help="Lua script to run (relative to repo root).",
        ),
    ] = "tools/perf/wfb_grpc_aggregate.lua",
) -> None:
    """Run a WFB gRPC profiling session using samply (Linux) and open the UI."""
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=0,
        env_templates=("GRPC_TARGET={GRPC_TARGET}",),
    )

    try:
        run_samply_profile(cfg, samply=SamplyConfig(open_ui=open_ui))
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None


@app.command()
def json_aggregate(
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
    ] = 128,
    script: Annotated[
        str,
        typer.Option(
            "--script",
            help="Lua script to run (relative to repo root).",
        ),
    ] = "examples/json_aggregate.lua",
) -> None:
    """
    Profile the HTTP json_aggregate example.

    Builds wrkr in profiling mode, starts testserver, warms up, then runs wrkr
    under a platform profiler to capture CPU stack traces.
    """
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=0,
        env_templates=("BASE_URL={BASE_URL}",),
    )
    try:
        run_profile(cfg)
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None


@app.command()
def grpc_aggregate_samply(
    sample_duration: Annotated[
        int,
        typer.Option(
            "--sample-duration",
            help="How long samply records stacks (seconds).",
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
    open_ui: Annotated[
        bool,
        typer.Option(
            "--open/--no-open",
            help="Open the samply UI after recording.",
        ),
    ] = True,
    script: Annotated[
        str,
        typer.Option(
            "--script",
            help="Lua script to run (relative to repo root).",
        ),
    ] = "examples/grpc_aggregate.lua",
) -> None:
    """Profile the gRPC grpc_aggregate example using samply."""
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=0,
        # examples/grpc_aggregate.lua currently uses examples/lib/example.lua which expects BASE_URL,
        # but the target we want is the gRPC endpoint provided as GRPC_TARGET.
        env_templates=(
            "BASE_URL={GRPC_TARGET}",
            "GRPC_TARGET={GRPC_TARGET}",
        ),
    )

    try:
        run_samply_profile(cfg, samply=SamplyConfig(open_ui=open_ui))
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None


@app.command()
def json_aggregate_samply(
    sample_duration: Annotated[
        int,
        typer.Option(
            "--sample-duration",
            help="How long samply records stacks (seconds).",
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
    open_ui: Annotated[
        bool,
        typer.Option(
            "--open/--no-open",
            help="Open the samply UI after recording.",
        ),
    ] = True,
    script: Annotated[
        str,
        typer.Option(
            "--script",
            help="Lua script to run (relative to repo root).",
        ),
    ] = "examples/json_aggregate.lua",
) -> None:
    """Profile the HTTP json_aggregate example using samply."""
    cfg = ProfileConfig(
        sample_duration_seconds=sample_duration,
        load_duration=load_duration,
        vus=vus,
        script=script,
        pre_sample_sleep_seconds=0,
        env_templates=("BASE_URL={BASE_URL}",),
    )

    try:
        run_samply_profile(cfg, samply=SamplyConfig(open_ui=open_ui))
    except ProfileError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        raise typer.Exit(code=1) from None
