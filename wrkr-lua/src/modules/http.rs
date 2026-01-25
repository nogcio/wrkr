use std::sync::Arc;
use std::time::Instant;

use mlua::{Lua, Table, Value};

use crate::Result;

mod opts;
mod result;
mod url;

use opts::parse_http_opts;
use result::HttpLuaResponse;
use url::{apply_params_owned, resolve_base_url};

fn create_http_module(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
) -> Result<Table> {
    let client = run_ctx.client.clone();
    let env_vars = run_ctx.env.clone();
    let metrics = run_ctx.metrics.clone();
    let request_metrics = run_ctx.request_metrics;

    // http.get(url, opts?) -> { status = 200, body = "...", error? = "..." }
    let http_tbl = lua.create_table()?;
    let http_get = {
        let client = client.clone();
        let env_vars = env_vars.clone();
        let metrics = metrics.clone();
        let metrics_ctx = metrics_ctx.clone();
        lua.create_async_function(move |lua, (url, opts): (String, Option<Table>)| {
            let client = client.clone();
            let env_vars = env_vars.clone();
            let metrics = metrics.clone();
            let metrics_ctx = metrics_ctx.clone();
            async move {
                let opts = parse_http_opts(opts).map_err(mlua::Error::external)?;
                let request_url =
                    resolve_base_url(&env_vars, apply_params_owned(url, &opts.params));

                let mut _tags = opts.tags;

                metrics_ctx.merge_scenario_tags_if_missing(
                    &mut _tags,
                    &["scenario", "protocol", "error_kind", "group"],
                );

                if let Some(group) = super::group::current_group(&lua)
                    && !_tags.iter().any(|(k, _)| k == "group")
                {
                    _tags.push(("group".to_string(), group));
                }

                let extra_tags: Vec<(&str, &str)> = _tags
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();

                let mut req = wrkr_http::HttpRequest::get_owned(request_url);
                req.headers = opts.headers;
                req.timeout = opts.timeout;
                let started = Instant::now();
                let res = client.request(req).await;
                let elapsed = started.elapsed();

                match res {
                    Ok(res) => {
                        request_metrics.record_request(
                            &metrics,
                            wrkr_core::RequestSample {
                                scenario: metrics_ctx.scenario(),
                                protocol: wrkr_core::Protocol::Http,
                                ok: true,
                                latency: elapsed,
                                bytes_received: res.bytes_received,
                                bytes_sent: res.bytes_sent,
                                error_kind: None,
                            },
                            &extra_tags,
                        );

                        HttpLuaResponse::ok(res).into_lua_table(&lua)
                    }
                    Err(err) => {
                        let kind = err.transport_error_kind().to_string();
                        request_metrics.record_request(
                            &metrics,
                            wrkr_core::RequestSample {
                                scenario: metrics_ctx.scenario(),
                                protocol: wrkr_core::Protocol::Http,
                                ok: false,
                                latency: elapsed,
                                bytes_received: 0,
                                bytes_sent: 0,
                                error_kind: Some(kind.as_str()),
                            },
                            &extra_tags,
                        );

                        HttpLuaResponse::err(err).into_lua_table(&lua)
                    }
                }
            }
        })?
    };
    http_tbl.set("get", http_get)?;

    // http.post(url, body, opts?)
    let http_post = {
        let client = client.clone();
        let env_vars = env_vars.clone();
        let metrics = metrics.clone();
        let metrics_ctx = metrics_ctx.clone();
        lua.create_async_function(
            move |lua, (url, body, opts): (String, Value, Option<Table>)| {
                let client = client.clone();
                let env_vars = env_vars.clone();
                let metrics = metrics.clone();
                let metrics_ctx = metrics_ctx.clone();
                async move {
                    let opts = parse_http_opts(opts).map_err(mlua::Error::external)?;
                    let request_url =
                        resolve_base_url(&env_vars, apply_params_owned(url, &opts.params));

                    let mut _tags = opts.tags;

                    metrics_ctx.merge_scenario_tags_if_missing(
                        &mut _tags,
                        &["scenario", "protocol", "error_kind", "group"],
                    );

                    if let Some(group) = super::group::current_group(&lua)
                        && !_tags.iter().any(|(k, _)| k == "group")
                    {
                        _tags.push(("group".to_string(), group));
                    }

                    let extra_tags: Vec<(&str, &str)> = _tags
                        .iter()
                        .map(|(k, v)| (k.as_str(), v.as_str()))
                        .collect();
                    let mut headers = opts.headers;

                    let (body, default_content_type) = match body {
                        Value::String(s) => (
                            bytes::Bytes::copy_from_slice(s.as_bytes().as_ref()),
                            "text/plain; charset=utf-8",
                        ),
                        v => (
                            bytes::Bytes::from(
                                crate::json_util::encode(&lua, v).map_err(mlua::Error::external)?,
                            ),
                            "application/json; charset=utf-8",
                        ),
                    };

                    if !headers
                        .iter()
                        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    {
                        headers
                            .push(("content-type".to_string(), default_content_type.to_string()));
                    }

                    let mut req = wrkr_http::HttpRequest::post_owned(request_url, body);
                    req.headers = headers;
                    req.timeout = opts.timeout;

                    let started = Instant::now();
                    let res = client.request(req).await;
                    let elapsed = started.elapsed();

                    match res {
                        Ok(res) => {
                            request_metrics.record_request(
                                &metrics,
                                wrkr_core::RequestSample {
                                    scenario: metrics_ctx.scenario(),
                                    protocol: wrkr_core::Protocol::Http,
                                    ok: true,
                                    latency: elapsed,
                                    bytes_received: res.bytes_received,
                                    bytes_sent: res.bytes_sent,
                                    error_kind: None,
                                },
                                &extra_tags,
                            );

                            HttpLuaResponse::ok(res).into_lua_table(&lua)
                        }
                        Err(err) => {
                            let kind = err.transport_error_kind().to_string();
                            request_metrics.record_request(
                                &metrics,
                                wrkr_core::RequestSample {
                                    scenario: metrics_ctx.scenario(),
                                    protocol: wrkr_core::Protocol::Http,
                                    ok: false,
                                    latency: elapsed,
                                    bytes_received: 0,
                                    bytes_sent: 0,
                                    error_kind: Some(kind.as_str()),
                                },
                                &extra_tags,
                            );

                            HttpLuaResponse::err(err).into_lua_table(&lua)
                        }
                    }
                }
            },
        )?
    };
    http_tbl.set("post", http_post)?;

    Ok(http_tbl)
}

pub(super) fn register_runtime(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
) -> Result<()> {
    let loader = {
        let run_ctx = run_ctx.clone();
        let metrics_ctx = metrics_ctx.clone();
        lua.create_function(move |lua, ()| {
            create_http_module(lua, run_ctx.clone(), metrics_ctx.clone())
                .map_err(mlua::Error::external)
        })?
    };
    super::preload_set(lua, "wrkr/http", loader)
}
