use mlua::{Lua, Thread, Value};
use std::time::Duration;
use std::time::Instant;

use crate::debugger;
use crate::loader::{chunk_name, configure_module_path};
use crate::modules;
use crate::{Error, Result};

pub async fn run_vu(ctx: wrkr_core::VuContext) -> Result<()> {
    let debugging = debugger::debugging_enabled();

    let init = (|| -> Result<(Lua, mlua::Function)> {
        let lua = if debugging {
            // `local-lua-debugger-vscode` requires the `debug` standard library.
            // `mlua::Lua::new()` is a safe mode that does not load `debug`.
            unsafe { Lua::unsafe_new() }
        } else {
            Lua::new()
        };

        configure_module_path(&lua, &ctx.run_ctx.script_path)?;
        modules::register(
            &lua,
            modules::RegisterContext {
                vu_id: ctx.vu_id,
                max_vus: ctx.max_vus,
                metrics_ctx: ctx.metrics_ctx.clone(),
                run_ctx: ctx.run_ctx.as_ref(),
            },
        )?;

        debugger::maybe_start_debugger(&lua);

        let chunk_name = chunk_name(&ctx.run_ctx.script_path);
        lua.load(&ctx.run_ctx.script).set_name(&chunk_name).exec()?;

        let exec_fn: mlua::Function = match lua.globals().get(ctx.exec.as_str())? {
            Value::Function(f) => f,
            _ if ctx.exec.eq("Default") => return Err(Error::MissingDefault),
            _ => return Err(Error::MissingExec(ctx.exec.to_string())),
        };

        Ok((lua, exec_fn))
    })();

    let (lua, exec_fn) = match init {
        Ok(v) => v,
        Err(err) => {
            let msg = err.to_string();
            {
                let mut guard = ctx
                    .init_error
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if guard.is_none() {
                    *guard = Some(msg);
                }
            }

            ctx.ready_barrier.wait().await;
            return Err(err);
        }
    };

    // Signal that this VU has finished initialization (Lua created, script loaded).
    ctx.ready_barrier.wait().await;
    // Block until the runner starts timing and opens the gate.
    ctx.start_signal.wait().await;

    let started = ctx
        .run_started
        .get()
        .copied()
        .unwrap_or_else(std::time::Instant::now);

    let _active_guard = ctx.enter_active_vu();

    let create_exec_coroutine: Option<mlua::Function> = if debugging {
        Some(
            lua.load(r#"return function(f) return coroutine.create(f) end"#)
                .set_name("wrkr_create_exec_coroutine")
                .eval()?,
        )
    } else {
        None
    };

    async fn run_one(
        create_exec_coroutine: Option<&mlua::Function>,
        exec_fn: &mlua::Function,
    ) -> Result<()> {
        if let Some(create_exec_coroutine) = create_exec_coroutine {
            // `mlua` runs async functions on a Lua thread created via the C API.
            // The VS Code lldebugger hooks Lua-created coroutines, so we create
            // the coroutine in Lua-land to ensure line breakpoints inside `Default()` bind.
            let thread: Thread = create_exec_coroutine.call(exec_fn.clone())?;

            // Drive the coroutine to completion (this also runs any Rust futures
            // yielded by async Rust callbacks, e.g. HTTP calls).
            thread.into_async::<()>(())?.await?;
        } else {
            exec_fn.call_async::<()>(()).await?;
        }

        Ok(())
    }

    match &ctx.work {
        wrkr_core::VuWork::Constant { gate } => {
            while gate.next() {
                let started = Instant::now();
                let res = run_one(create_exec_coroutine.as_ref(), &exec_fn).await;
                let elapsed = started.elapsed();
                ctx.record_iteration(elapsed, res.is_ok());
                res?;
            }
        }
        wrkr_core::VuWork::RampingVus { schedule } => loop {
            let elapsed = started.elapsed();
            if schedule.is_done(elapsed) {
                break;
            }

            let target = schedule.target_at(elapsed);
            if ctx.scenario_vu > target {
                let wait = schedule.next_recheck_in(elapsed, ctx.scenario_vu);
                tokio::time::sleep(wait.max(Duration::from_millis(1))).await;
                continue;
            }

            let started = Instant::now();
            let res = run_one(create_exec_coroutine.as_ref(), &exec_fn).await;
            let elapsed = started.elapsed();
            ctx.record_iteration(elapsed, res.is_ok());
            res?;
        },
        wrkr_core::VuWork::RampingArrivalRate {
            schedule, pacer, ..
        } => {
            loop {
                let elapsed = started.elapsed();
                if schedule.is_done(elapsed) && pacer.is_done() {
                    // No more tokens will be scheduled; drain any remaining then stop.
                    if !pacer.claim_next().await {
                        break;
                    }
                    let started = Instant::now();
                    let res = run_one(create_exec_coroutine.as_ref(), &exec_fn).await;
                    let elapsed = started.elapsed();
                    ctx.record_iteration(elapsed, res.is_ok());
                    res?;
                    continue;
                }

                // Only some VUs are active at a time (adaptive policy inside the pacer).
                if ctx.scenario_vu > pacer.active_vus() {
                    pacer.wait_for_update().await;
                    continue;
                }

                if !pacer.claim_next().await {
                    break;
                }

                let started = Instant::now();
                let res = run_one(create_exec_coroutine.as_ref(), &exec_fn).await;
                let elapsed = started.elapsed();
                ctx.record_iteration(elapsed, res.is_ok());
                res?;
            }
        }
    }

    Ok(())
}
