use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use mlua::{Lua, Table, Value};

use crate::Result;
use crate::group_api;
use crate::grpc_shared;
use crate::value_util::{Int64Repr, lua_to_value, value_to_lua};

fn resolve_path(script_path: Option<&Path>, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        return path;
    }

    if let Some(script_path) = script_path
        && let Some(dir) = script_path.parent()
    {
        return dir.join(path);
    }

    path
}

fn parse_duration(v: &str) -> mlua::Result<Duration> {
    humantime::parse_duration(v)
        .map_err(|e| mlua::Error::external(format!("invalid duration '{v}': {e}")))
}

fn parse_metadata(opts: &Table) -> Result<Vec<(String, String)>> {
    let Ok(md_tbl) = opts.get::<Table>("metadata") else {
        return Ok(Vec::new());
    };

    let mut out: Vec<(String, String)> = Vec::new();

    for pair in md_tbl.pairs::<Value, Value>() {
        let (k, v) = pair?;
        let key = match k {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };

        match v {
            Value::String(s) => {
                out.push((key, s.to_string_lossy().to_string()));
            }
            Value::Table(arr) => {
                for vv in arr.sequence_values::<Value>() {
                    let vv = vv?;
                    if let Value::String(s) = vv {
                        out.push((key.clone(), s.to_string_lossy().to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(out)
}

fn parse_tags(opts: &Table) -> Result<Vec<(String, String)>> {
    let Ok(tags_tbl) = opts.get::<Table>("tags") else {
        return Ok(Vec::new());
    };

    let mut out: Vec<(String, String)> = Vec::new();
    for pair in tags_tbl.pairs::<Value, Value>() {
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
        out.push((k, v));
    }
    Ok(out)
}

fn grpc_error_kind(err: &wrkr_core::GrpcError) -> wrkr_core::GrpcTransportErrorKind {
    err.transport_error_kind()
}

pub fn create_grpc_module(
    lua: &Lua,
    script_path: Option<&Path>,
    max_vus: u64,
    stats: Arc<wrkr_core::runner::RunStats>,
) -> Result<Table> {
    let grpc_tbl = lua.create_table()?;

    // grpc.Client.new(opts?) -> client
    let client_tbl = lua.create_table()?;

    let script_path = script_path.map(PathBuf::from);

    let new_fn = {
        let stats = stats.clone();
        let script_path = script_path.clone();
        lua.create_function(move |lua, opts: Option<Table>| {
            let mut pool_size = grpc_shared::default_pool_size(max_vus);
            if let Some(opts) = opts
                && let Some(pool_val) = opts.get::<Option<Value>>("pool_size")?
            {
                let raw_value: i64 = match pool_val {
                    Value::Integer(i) => i,
                    Value::Number(n) => {
                        if !n.is_finite() || n.fract() != 0.0 {
                            return Err(mlua::Error::external(
                                "grpc.Client.new: pool_size must be a finite integer",
                            ));
                        }
                        n as i64
                    }
                    _ => {
                        return Err(mlua::Error::external(
                            "grpc.Client.new: pool_size must be a number",
                        ));
                    }
                };

                if raw_value <= 0 {
                    return Err(mlua::Error::external(
                        "grpc.Client.new: pool_size must be a positive integer",
                    ));
                }

                let requested: usize = usize::try_from(raw_value).map_err(|_| {
                    mlua::Error::external(
                        "grpc.Client.new: pool_size is too large for this platform",
                    )
                })?;

                let max_pool = (max_vus as usize).clamp(1, 1024);
                if requested > max_pool {
                    return Err(mlua::Error::external(format!(
                        "grpc.Client.new: pool_size must be between 1 and {max_pool}",
                    )));
                }

                pool_size = requested;
            }

            let shared = grpc_shared::get_or_create(&stats, pool_size);
            let client_obj = lua.create_table()?;

            // load(paths, file) -> true | (nil, err)
            let load_fn = {
                let shared = shared.clone();
                let script_path = script_path.clone();
                lua.create_function(move |_lua, (_this, paths, file): (Table, Table, String)| {
                    let script_path = script_path.as_deref();

                    let mut include_paths: Vec<PathBuf> = Vec::new();
                    for v in paths.sequence_values::<String>() {
                        let p = v?;
                        include_paths.push(resolve_path(script_path, &p));
                    }

                    let proto_file = resolve_path(script_path, &file);
                    shared
                        .load(include_paths, proto_file)
                        .map_err(mlua::Error::external)?;
                    Ok(true)
                })?
            };

            // connect(target, opts?) -> true | (nil, err)
            let connect_fn = {
                let shared = shared.clone();
                lua.create_async_function(
                    move |lua, (_this, target, opts): (Table, String, Option<Table>)| {
                        let shared = shared.clone();
                        async move {
                            let mut options = wrkr_core::GrpcConnectOptions::default();

                            if let Some(opts) = opts {
                                if let Ok(timeout) = opts.get::<String>("timeout") {
                                    options.timeout = Some(parse_duration(&timeout)?);
                                }

                                if let Ok(tls_tbl) = opts.get::<Table>("tls") {
                                    let mut tls = wrkr_core::GrpcTlsConfig::default();

                                    if let Ok(domain) = tls_tbl.get::<String>("server_name") {
                                        tls.domain_name = Some(domain);
                                    }
                                    if let Ok(skip) = tls_tbl.get::<bool>("insecure_skip_verify") {
                                        tls.insecure_skip_verify = skip;
                                    }
                                    if let Ok(ca) = tls_tbl.get::<mlua::String>("ca") {
                                        tls.ca_pem = Some(ca.as_bytes().to_vec());
                                    }
                                    if let Ok(cert) = tls_tbl.get::<mlua::String>("cert") {
                                        tls.identity_pem = Some(cert.as_bytes().to_vec());
                                    }
                                    if let Ok(key) = tls_tbl.get::<mlua::String>("key") {
                                        tls.identity_key_pem = Some(key.as_bytes().to_vec());
                                    }

                                    options.tls = Some(tls);
                                }
                            }

                            match shared.connect(target, options).await {
                                Ok(()) => {
                                    Ok(mlua::MultiValue::from_vec(vec![Value::Boolean(true)]))
                                }
                                Err(err) => Ok(mlua::MultiValue::from_vec(vec![
                                    Value::Nil,
                                    Value::String(lua.create_string(err.as_bytes())?),
                                ])),
                            }
                        }
                    },
                )?
            };

            // invoke(full_method, req, opts?) -> res_tbl (never throws on runtime errors)
            // If `req` is a Lua string, it's treated as protobuf-encoded request bytes.
            // Otherwise `req` is converted from Lua -> wrkr_value::Value and encoded.
            let invoke_fn = {
                let shared = shared.clone();
                let stats = stats.clone();
                lua.create_async_function(
                    move |lua,
                          (_this, full_method, req, opts): (
                        Table,
                        mlua::String,
                        Value,
                        Option<Table>,
                    )| {
                        let shared = shared.clone();
                        let stats = stats.clone();
                        async move {
                            let started = std::time::Instant::now();
                            let client = shared.client();

                            let t = lua.create_table()?;

                            let Some(client) = client else {
                                t.set("ok", false)?;
                                t.set("status", Value::Nil)?;
                                t.set("error_kind", "not_connected")?;
                                t.set("error", "grpc client: call connect() first")?;
                                return Ok(t);
                            };

                            let full_method = match full_method.to_str() {
                                Ok(s) => s,
                                Err(_) => {
                                    t.set("ok", false)?;
                                    t.set("status", Value::Nil)?;
                                    t.set("error_kind", "invalid_method")?;
                                    t.set("error", "grpc client: method name must be utf-8")?;
                                    return Ok(t);
                                }
                            };

                            let full_method_str: &str = full_method.as_ref();

                            let method = match shared.method(full_method_str) {
                                Ok(m) => m,
                                Err(_) => {
                                    t.set("ok", false)?;
                                    t.set("status", Value::Nil)?;
                                    t.set("error_kind", "not_loaded")?;
                                    t.set("error", "grpc client: call load() first")?;
                                    return Ok(t);
                                }
                            };

                            let mut tags: Vec<(String, String)> = Vec::new();
                            let mut name: Option<String> = None;
                            let mut timeout: Option<Duration> = None;
                            let mut metadata: Vec<(String, String)> = Vec::new();
                            let mut int64_repr = Int64Repr::Integer;

                            if let Some(opts) = opts {
                                tags = parse_tags(&opts).map_err(mlua::Error::external)?;
                                name = opts.get::<String>("name").ok();
                                if let Ok(timeout_str) = opts.get::<String>("timeout") {
                                    timeout = Some(parse_duration(&timeout_str)?);
                                }
                                metadata = parse_metadata(&opts).map_err(mlua::Error::external)?;

                                if let Ok(int64_str) = opts.get::<String>("int64") {
                                    int64_repr = match int64_str.as_str() {
                                        "integer" => Int64Repr::Integer,
                                        "string" => Int64Repr::String,
                                        _ => {
                                            return Err(mlua::Error::external(
                                                "grpc invoke opts.int64 must be 'integer' or 'string'",
                                            ));
                                        }
                                    };
                                }
                            }

                            let metric_name = name.as_deref().unwrap_or(full_method_str);

                            if let Some(group) = group_api::current_group(&lua)
                                && !tags.iter().any(|(k, _)| k == "group")
                            {
                                tags.push(("group".to_string(), group));
                            }

                            let invoke_opts = wrkr_core::GrpcInvokeOptions { timeout, metadata };

                            let res = match req {
                                Value::String(req_bytes) => {
                                    let bytes = bytes::Bytes::copy_from_slice(
                                        req_bytes.as_bytes().as_ref(),
                                    );
                                    client
                                        .unary_bytes(method.as_ref(), bytes, invoke_opts)
                                        .await
                                }
                                other => {
                                    let req_value =
                                        match lua_to_value(&lua, other, Int64Repr::String) {
                                            Ok(v) => v,
                                            Err(err) => {
                                                t.set("ok", false)?;
                                                t.set("status", Value::Nil)?;
                                                t.set("error_kind", "encode")?;
                                                t.set("error", err.to_string())?;
                                                return Ok(t);
                                            }
                                        };

                                    client.unary(method.as_ref(), req_value, invoke_opts).await
                                }
                            };

                            match res {
                                Ok(res) => {
                                    stats.record_grpc_request(
                                        wrkr_core::runner::GrpcRequestMeta {
                                            method: wrkr_core::runner::GrpcCallKind::Unary,
                                            name: metric_name,
                                            status: res.status,
                                            transport_error_kind: res.transport_error_kind,
                                            elapsed: res.elapsed,
                                            bytes_received: res.bytes_received,
                                            bytes_sent: res.bytes_sent,
                                        },
                                        &tags,
                                    );

                                    t.set("ok", res.ok)?;
                                    if let Some(status) = res.status {
                                        t.set("status", status)?;
                                    } else {
                                        t.set("status", Value::Nil)?;
                                    }
                                    if let Some(msg) = res.message {
                                        t.set("message", msg)?;
                                    }
                                    if let Some(err) = res.error {
                                        t.set("error", err)?;
                                    }
                                    if let Some(kind) = res.transport_error_kind {
                                        t.set("error_kind", kind.to_string())?;
                                    }

                                    let resp =
                                        value_to_lua(&lua, &res.response, int64_repr)
                                        .map_err(mlua::Error::external)?;
                                    t.set("response", resp)?;

                                    Ok(t)
                                }
                                Err(err) => {
                                    let kind = grpc_error_kind(&err);

                                    // best-effort metrics on transport errors
                                    stats.record_grpc_request(
                                        wrkr_core::runner::GrpcRequestMeta {
                                            method: wrkr_core::runner::GrpcCallKind::Unary,
                                            name: metric_name,
                                            status: None,
                                            transport_error_kind: Some(kind),
                                            elapsed: started.elapsed(),
                                            bytes_received: 0,
                                            bytes_sent: 0,
                                        },
                                        &tags,
                                    );

                                    t.set("ok", false)?;
                                    t.set("status", Value::Nil)?;
                                    t.set("error_kind", kind.to_string())?;
                                    t.set("error", err.to_string())?;
                                    Ok(t)
                                }
                            }
                        }
                    },
                )?
            };

            // encode(full_method, req) -> bytes | (nil, err)
            // Encodes a request message to protobuf bytes, allowing callers to cache/reuse the
            // bytes across many invocations (avoids repeated Lua->Value->protobuf work).
            let encode_fn = {
                let shared = shared.clone();
                lua.create_function(
                    move |lua, (_this, full_method, req): (Table, mlua::String, Value)| {
                        let full_method =
                            match full_method.to_str() {
                                Ok(s) => s,
                                Err(_) => {
                                    return Ok(mlua::MultiValue::from_vec(vec![
                                        Value::Nil,
                                        Value::String(lua.create_string(
                                            "grpc client: method name must be utf-8",
                                        )?),
                                    ]));
                                }
                            };

                        let method = match shared.method(full_method.as_ref()) {
                            Ok(m) => m,
                            Err(_) => {
                                return Ok(mlua::MultiValue::from_vec(vec![
                                    Value::Nil,
                                    Value::String(
                                        lua.create_string("grpc client: call load() first")?,
                                    ),
                                ]));
                            }
                        };

                        let req_value = match lua_to_value(lua, req, Int64Repr::String) {
                            Ok(v) => v,
                            Err(err) => {
                                return Ok(mlua::MultiValue::from_vec(vec![
                                    Value::Nil,
                                    Value::String(lua.create_string(err.to_string().as_bytes())?),
                                ]));
                            }
                        };

                        match wrkr_core::grpc_encode_unary_request(method.as_ref(), &req_value) {
                            Ok(bytes) => Ok(mlua::MultiValue::from_vec(vec![Value::String(
                                lua.create_string(bytes.as_ref())?,
                            )])),
                            Err(err) => Ok(mlua::MultiValue::from_vec(vec![
                                Value::Nil,
                                Value::String(lua.create_string(err.to_string().as_bytes())?),
                            ])),
                        }
                    },
                )?
            };

            client_obj.set("load", load_fn)?;
            client_obj.set("connect", connect_fn)?;
            client_obj.set("invoke", invoke_fn)?;
            client_obj.set("encode", encode_fn)?;

            Ok::<_, mlua::Error>(client_obj)
        })?
    };

    client_tbl.set("new", new_fn)?;
    grpc_tbl.set("Client", client_tbl)?;

    Ok(grpc_tbl)
}
