use mlua::{Lua, Table, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{cell::RefCell, collections::HashMap};

use crate::group_api;
use crate::json_util;
use crate::{Error, Result};

#[derive(Debug, Default)]
struct HttpRequestOptions {
    headers: Vec<(String, String)>,
    params: Vec<(String, String)>,
    timeout: Option<Duration>,
    tags: Vec<(String, String)>,
    name: Option<String>,
}

pub fn create_http_module(
    lua: &Lua,
    scenario: Arc<str>,
    client: Arc<wrkr_core::HttpClient>,
    stats: Arc<wrkr_core::runner::RunStats>,
) -> Result<Table> {
    // http.get(url, opts?) -> { status = 200, body = "...", error? = "..." }
    let http_tbl = lua.create_table()?;
    let http_get = {
        let scenario = scenario.clone();
        let client = client.clone();
        let stats = stats.clone();
        lua.create_async_function(move |lua, (url, opts): (String, Option<Table>)| {
            let scenario = scenario.clone();
            let client = client.clone();
            let stats = stats.clone();
            async move {
                let opts = parse_http_opts(opts).map_err(mlua::Error::external)?;
                let request_url = apply_params_owned(url, &opts.params);
                let metric_name = opts.name.clone().unwrap_or_else(|| request_url.clone());

                let mut tags = opts.tags;
                if let Some(group) = group_api::current_group(&lua)
                    && !tags.iter().any(|(k, _)| k == "group")
                {
                    tags.push(("group".to_string(), group));
                }

                let started = Instant::now();
                let mut req = wrkr_core::HttpRequest::get_owned(request_url);
                req.headers = opts.headers;
                req.timeout = opts.timeout;

                let bytes_sent = wrkr_core::estimate_http_request_bytes(&req).unwrap_or(0);

                let res = client.request(req).await;
                let elapsed = started.elapsed();

                match res {
                    Ok(res) => {
                        let bytes_received = res.bytes_received;
                        stats.record_http_request_scoped(
                            scenario.as_ref(),
                            wrkr_core::runner::HttpRequestMeta {
                                method: "GET",
                                name: &metric_name,
                                status: Some(res.status),
                                transport_error_kind: None,
                                elapsed,
                                bytes_received,
                                bytes_sent,
                            },
                            &tags,
                        );

                        let t = lua.create_table()?;
                        t.set("status", res.status)?;
                        let body = res.body_utf8().unwrap_or("");
                        t.set("body", body)?;
                        Ok(t)
                    }
                    Err(err) => {
                        let err_kind = err.transport_error_kind();
                        stats.record_http_request_scoped(
                            scenario.as_ref(),
                            wrkr_core::runner::HttpRequestMeta {
                                method: "GET",
                                name: &metric_name,
                                status: None,
                                transport_error_kind: Some(err_kind),
                                elapsed,
                                bytes_received: 0,
                                bytes_sent,
                            },
                            &tags,
                        );

                        let t = lua.create_table()?;
                        t.set("status", 0)?;
                        t.set("body", "")?;
                        t.set("error", err.to_string())?;
                        Ok(t)
                    }
                }
            }
        })?
    };
    http_tbl.set("get", http_get)?;

    // http.post(url, body, opts?)
    let http_post = {
        let scenario = scenario.clone();
        let client = client.clone();
        let stats = stats.clone();
        lua.create_async_function(
            move |lua, (url, body, opts): (String, Value, Option<Table>)| {
                let scenario = scenario.clone();
                let client = client.clone();
                let stats = stats.clone();
                async move {
                    let opts = parse_http_opts(opts).map_err(mlua::Error::external)?;
                    let request_url = apply_params_owned(url, &opts.params);
                    let metric_name = opts.name.clone().unwrap_or_else(|| request_url.clone());

                    let mut tags = opts.tags;
                    if let Some(group) = group_api::current_group(&lua)
                        && !tags.iter().any(|(k, _)| k == "group")
                    {
                        tags.push(("group".to_string(), group));
                    }
                    let mut headers = opts.headers;

                    let (body, default_content_type) = match body {
                        Value::String(s) => (
                            bytes::Bytes::copy_from_slice(s.as_bytes().as_ref()),
                            "text/plain; charset=utf-8",
                        ),
                        v => (
                            bytes::Bytes::from(
                                json_util::encode(&lua, v).map_err(mlua::Error::external)?,
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

                    let started = Instant::now();
                    let mut req = wrkr_core::HttpRequest::post_owned(request_url, body);
                    req.headers = headers;
                    req.timeout = opts.timeout;

                    let bytes_sent = wrkr_core::estimate_http_request_bytes(&req).unwrap_or(0);

                    let res = client.request(req).await;
                    let elapsed = started.elapsed();

                    match res {
                        Ok(res) => {
                            let bytes_received = res.bytes_received;
                            stats.record_http_request_scoped(
                                scenario.as_ref(),
                                wrkr_core::runner::HttpRequestMeta {
                                    method: "POST",
                                    name: &metric_name,
                                    status: Some(res.status),
                                    transport_error_kind: None,
                                    elapsed,
                                    bytes_received,
                                    bytes_sent,
                                },
                                &tags,
                            );

                            let t = lua.create_table()?;
                            t.set("status", res.status)?;
                            let body = res.body_utf8().unwrap_or("");
                            t.set("body", body)?;
                            Ok(t)
                        }
                        Err(err) => {
                            let err_kind = err.transport_error_kind();
                            stats.record_http_request_scoped(
                                scenario.as_ref(),
                                wrkr_core::runner::HttpRequestMeta {
                                    method: "POST",
                                    name: &metric_name,
                                    status: None,
                                    transport_error_kind: Some(err_kind),
                                    elapsed,
                                    bytes_received: 0,
                                    bytes_sent,
                                },
                                &tags,
                            );

                            let t = lua.create_table()?;
                            t.set("status", 0)?;
                            t.set("body", "")?;
                            t.set("error", err.to_string())?;
                            Ok(t)
                        }
                    }
                }
            },
        )?
    };
    http_tbl.set("post", http_post)?;

    Ok(http_tbl)
}

pub fn create_check_function(
    lua: &Lua,
    scenario: Arc<str>,
    stats: Arc<wrkr_core::runner::RunStats>,
) -> Result<mlua::Function> {
    // check(res, { ["name"] = function(r) return ... end, ... }) -> bool
    let check_fn = {
        let scenario = scenario.clone();
        let handles_cache: RefCell<HashMap<Box<str>, wrkr_core::runner::CheckHandle>> =
            RefCell::new(HashMap::new());

        lua.create_function(move |_lua, (res, checks): (Table, Table)| {
            let scenario = scenario.clone();
            let mut all_ok = true;
            for pair in checks.pairs::<Value, Value>() {
                let (k, v) = pair?;
                let f = match v {
                    Value::Function(f) => f,
                    _ => continue,
                };

                match k {
                    Value::String(s) => {
                        let ok: bool = f.call(res.clone())?;
                        if let Ok(name_borrowed) = s.to_str() {
                            let name = name_borrowed.as_ref();
                            let handle = {
                                let mut cache = handles_cache.borrow_mut();
                                if let Some(h) = cache.get(name) {
                                    h.clone()
                                } else {
                                    let h = stats.check_handle(name);
                                    cache.insert(name.to_owned().into_boxed_str(), h.clone());
                                    h
                                }
                            };

                            stats.record_check_handle(&handle, ok);
                            stats.record_check_for_scenario(scenario.as_ref(), name, ok);
                        } else {
                            // Note: this allocates, but only for truly non-UTF8 keys.
                            let name_owned = s.to_string_lossy();
                            let name = name_owned.as_str();

                            let handle = {
                                let mut cache = handles_cache.borrow_mut();
                                if let Some(h) = cache.get(name) {
                                    h.clone()
                                } else {
                                    let h = stats.check_handle(name);
                                    cache.insert(name.to_owned().into_boxed_str(), h.clone());
                                    h
                                }
                            };

                            stats.record_check_handle(&handle, ok);
                            stats.record_check_for_scenario(scenario.as_ref(), name, ok);
                        }
                        if !ok {
                            all_ok = false;
                        }
                    }
                    _ => {
                        let ok: bool = f.call(res.clone())?;
                        let handle = {
                            let mut cache = handles_cache.borrow_mut();
                            if let Some(h) = cache.get("<unnamed>") {
                                h.clone()
                            } else {
                                let h = stats.check_handle("<unnamed>");
                                cache.insert("<unnamed>".to_owned().into_boxed_str(), h.clone());
                                h
                            }
                        };

                        stats.record_check_handle(&handle, ok);
                        stats.record_check_for_scenario(scenario.as_ref(), "<unnamed>", ok);
                        if !ok {
                            all_ok = false;
                        }
                    }
                }
            }
            Ok(all_ok)
        })?
    };
    Ok(check_fn)
}

fn parse_http_opts(opts: Option<Table>) -> Result<HttpRequestOptions> {
    let Some(opts) = opts else {
        return Ok(HttpRequestOptions {
            headers: Vec::new(),
            params: Vec::new(),
            timeout: None,
            tags: Vec::new(),
            name: None,
        });
    };

    let mut headers = Vec::new();
    if let Ok(hdrs) = opts.get::<Table>("headers") {
        for pair in hdrs.pairs::<Value, Value>() {
            let (k, v) = pair?;
            let k = match k {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => continue,
            };
            let v = match v {
                Value::String(s) => s.to_string_lossy().to_string(),
                Value::Integer(i) => i.to_string(),
                Value::Number(n) => n.to_string(),
                _ => continue,
            };
            headers.push((k, v));
        }
    }

    let mut params = Vec::new();
    if let Ok(p) = opts.get::<Table>("params") {
        for pair in p.pairs::<Value, Value>() {
            let (k, v) = pair?;
            let k = match k {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => continue,
            };
            let v = match v {
                Value::String(s) => s.to_string_lossy().to_string(),
                Value::Integer(i) => i.to_string(),
                Value::Number(n) => n.to_string(),
                _ => continue,
            };
            params.push((k, v));
        }
    }

    let timeout = match opts.get::<Value>("timeout").ok() {
        Some(Value::Nil) | None => None,
        Some(Value::Number(n)) if n > 0.0 => Some(Duration::from_secs_f64(n)),
        Some(Value::Integer(i)) if i > 0 => Some(Duration::from_secs(i as u64)),
        Some(Value::String(s)) => {
            let s = s.to_string_lossy();
            Some(humantime::parse_duration(&s).map_err(|_| Error::InvalidDuration)?)
        }
        _ => return Err(Error::InvalidDuration),
    };

    let mut tags = Vec::new();
    if let Ok(t) = opts.get::<Table>("tags") {
        for pair in t.pairs::<Value, Value>() {
            let (k, v) = pair?;
            let k = match k {
                Value::String(s) => s.to_string_lossy().to_string(),
                _ => continue,
            };
            let v = match v {
                Value::String(s) => s.to_string_lossy().to_string(),
                Value::Integer(i) => i.to_string(),
                Value::Number(n) => n.to_string(),
                Value::Boolean(b) => b.to_string(),
                _ => continue,
            };
            tags.push((k, v));
        }
    }

    let name = match opts.get::<Value>("name").ok() {
        None | Some(Value::Nil) => None,
        Some(Value::String(s)) => Some(s.to_string_lossy().to_string()),
        _ => return Err(Error::InvalidHttpName),
    };

    Ok(HttpRequestOptions {
        headers,
        params,
        timeout,
        tags,
        name,
    })
}

fn apply_params_owned(url: String, params: &[(String, String)]) -> String {
    if params.is_empty() {
        return url;
    }

    let Ok(mut u) = url::Url::parse(&url) else {
        return url;
    };

    {
        let mut qp = u.query_pairs_mut();
        for (k, v) in params {
            qp.append_pair(k, v);
        }
    }

    u.to_string()
}
