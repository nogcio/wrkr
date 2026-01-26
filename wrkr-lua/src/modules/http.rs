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

fn has_header(headers: &[(String, String)], key: &str) -> bool {
    headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(key))
}

#[derive(Clone)]
struct HttpRuntime {
    client: Arc<wrkr_http::HttpClient>,
    env_vars: wrkr_core::EnvVars,
    metrics: Arc<wrkr_metrics::Registry>,
    request_metrics: wrkr_core::RequestMetricIds,
    metrics_ctx: wrkr_core::MetricsContext,
}

async fn request_impl(
    lua: &Lua,
    rt: &HttpRuntime,
    method: wrkr_http::Method,
    url: String,
    body: Option<Value>,
    opts: Option<Table>,
) -> mlua::Result<Table> {
    let opts = parse_http_opts(opts).map_err(mlua::Error::external)?;
    let request_url = resolve_base_url(&rt.env_vars, apply_params_owned(url, &opts.params));

    let mut tags = opts.tags;

    // Always record the HTTP method as a stable metric tag.
    tags.retain(|(k, _)| k != "method");
    tags.push(("method".to_string(), method.as_str().to_string()));

    if let Some(name) = opts.name {
        tags.retain(|(k, _)| k != "name");
        tags.push(("name".to_string(), name));
    }

    rt.metrics_ctx.merge_scenario_tags_if_missing(
        &mut tags,
        &["scenario", "protocol", "error_kind", "group"],
    );

    if let Some(group) = super::group::current_group(lua)
        && !tags.iter().any(|(k, _)| k == "group")
    {
        tags.push(("group".to_string(), group));
    }

    let extra_tags: Vec<(&str, &str)> =
        tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let mut headers = opts.headers;
    let body_bytes = match body {
        None | Some(Value::Nil) => bytes::Bytes::new(),
        Some(Value::String(s)) => {
            if !has_header(&headers, "content-type") {
                headers.push((
                    "content-type".to_string(),
                    "text/plain; charset=utf-8".to_string(),
                ));
            }
            bytes::Bytes::copy_from_slice(s.as_bytes().as_ref())
        }
        Some(v) => {
            if !has_header(&headers, "content-type") {
                headers.push((
                    "content-type".to_string(),
                    "application/json; charset=utf-8".to_string(),
                ));
            }
            bytes::Bytes::from(crate::json_util::encode(lua, v).map_err(mlua::Error::external)?)
        }
    };

    let req = wrkr_http::HttpRequest {
        method,
        url: request_url,
        headers,
        body: body_bytes,
        timeout: opts.timeout,
    };

    let started = Instant::now();
    let res = rt.client.request(req).await;
    let elapsed = started.elapsed();

    match res {
        Ok(res) => {
            rt.request_metrics.record_request(
                &rt.metrics,
                wrkr_core::RequestSample {
                    scenario: rt.metrics_ctx.scenario(),
                    protocol: wrkr_core::Protocol::Http,
                    ok: true,
                    latency: elapsed,
                    bytes_received: res.bytes_received,
                    bytes_sent: res.bytes_sent,
                    error_kind: None,
                },
                &extra_tags,
            );

            HttpLuaResponse::ok(res).into_lua_table(lua)
        }
        Err(err) => {
            let kind = err.transport_error_kind().to_string();
            rt.request_metrics.record_request(
                &rt.metrics,
                wrkr_core::RequestSample {
                    scenario: rt.metrics_ctx.scenario(),
                    protocol: wrkr_core::Protocol::Http,
                    ok: false,
                    latency: elapsed,
                    bytes_received: 0,
                    bytes_sent: 0,
                    error_kind: Some(kind.as_str()),
                },
                &extra_tags,
            );

            HttpLuaResponse::err(err).into_lua_table(lua)
        }
    }
}

fn create_http_module(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
) -> Result<Table> {
    let http_tbl = lua.create_table()?;
    let rt = HttpRuntime {
        client: run_ctx.client.clone(),
        env_vars: run_ctx.env.clone(),
        metrics: run_ctx.metrics.clone(),
        request_metrics: run_ctx.request_metrics,
        metrics_ctx: metrics_ctx.clone(),
    };

    // http.get(url, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(move |lua, (url, opts): (String, Option<Table>)| {
            let rt = rt.clone();
            async move { request_impl(&lua, &rt, wrkr_http::Method::GET, url, None, opts).await }
        })?;
        http_tbl.set("get", f)?;
    }

    // http.post(url, body, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(
            move |lua, (url, body, opts): (String, Value, Option<Table>)| {
                let rt = rt.clone();
                async move {
                    request_impl(&lua, &rt, wrkr_http::Method::POST, url, Some(body), opts).await
                }
            },
        )?;
        http_tbl.set("post", f)?;
    }

    // http.put(url, body, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(
            move |lua, (url, body, opts): (String, Value, Option<Table>)| {
                let rt = rt.clone();
                async move {
                    request_impl(&lua, &rt, wrkr_http::Method::PUT, url, Some(body), opts).await
                }
            },
        )?;
        http_tbl.set("put", f)?;
    }

    // http.patch(url, body, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(
            move |lua, (url, body, opts): (String, Value, Option<Table>)| {
                let rt = rt.clone();
                async move {
                    request_impl(&lua, &rt, wrkr_http::Method::PATCH, url, Some(body), opts).await
                }
            },
        )?;
        http_tbl.set("patch", f)?;
    }

    // http.delete(url, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(move |lua, (url, opts): (String, Option<Table>)| {
            let rt = rt.clone();
            async move { request_impl(&lua, &rt, wrkr_http::Method::DELETE, url, None, opts).await }
        })?;
        http_tbl.set("delete", f)?;
    }

    // http.head(url, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(move |lua, (url, opts): (String, Option<Table>)| {
            let rt = rt.clone();
            async move { request_impl(&lua, &rt, wrkr_http::Method::HEAD, url, None, opts).await }
        })?;
        http_tbl.set("head", f)?;
    }

    // http.options(url, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(move |lua, (url, opts): (String, Option<Table>)| {
            let rt = rt.clone();
            async move {
                request_impl(&lua, &rt, wrkr_http::Method::OPTIONS, url, None, opts).await
            }
        })?;
        http_tbl.set("options", f)?;
    }

    // http.request(method, url, body?, opts?) -> res
    {
        let rt = rt.clone();
        let f = lua.create_async_function(
            move |lua, (method, url, body, opts): (String, String, Option<Value>, Option<Table>)| {
                let rt = rt.clone();
                async move {
                    let m = wrkr_http::Method::from_bytes(method.as_bytes())
                        .map_err(mlua::Error::external)?;
                    request_impl(&lua, &rt, m, url, body, opts).await
                }
            },
        )?;
        http_tbl.set("request", f)?;
    }

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
