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

pub fn create_http_module(lua: &Lua, client: Arc<wrkr_core::HttpClient>) -> Result<Table> {
    // http.get(url, opts?) -> { status = 200, body = "...", error? = "..." }
    let http_tbl = lua.create_table()?;
    let http_get = {
        let client = client.clone();
        lua.create_async_function(move |lua, (url, opts): (String, Option<Table>)| {
            let client = client.clone();
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
                        let t = lua.create_table()?;
                        t.set("status", res.status)?;
                        let body = res.body_utf8().unwrap_or("");
                        t.set("body", body)?;
                        Ok(t)
                    }
                    Err(err) => {
                        let err_kind = err.transport_error_kind();
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
        let client = client.clone();
        lua.create_async_function(
            move |lua, (url, body, opts): (String, Value, Option<Table>)| {
                let client = client.clone();
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

                    let mut req = wrkr_core::HttpRequest::post_owned(request_url, body);
                    req.headers = headers;
                    req.timeout = opts.timeout;

                    let res = client.request(req).await;

                    match res {
                        Ok(res) => {
                            let t = lua.create_table()?;
                            t.set("status", res.status)?;
                            let body = res.body_utf8().unwrap_or("");
                            t.set("body", body)?;
                            Ok(t)
                        }
                        Err(err) => {
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
