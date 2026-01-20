use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mlua::{Lua, Table, Value};

use crate::Result;
use crate::group_api;
use crate::value_util::{Int64Repr, lua_to_value, value_to_lua};

#[derive(Debug, Default)]
struct GrpcClientState {
    schema: Option<wrkr_core::ProtoSchema>,
    client: Option<wrkr_core::GrpcClient>,
}

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
    stats: Arc<wrkr_core::runner::RunStats>,
) -> Result<Table> {
    let grpc_tbl = lua.create_table()?;

    // grpc.Client.new() -> client
    let client_tbl = lua.create_table()?;

    let script_path = script_path.map(PathBuf::from);

    let new_fn = {
        let stats = stats.clone();
        let script_path = script_path.clone();
        lua.create_function(move |lua, ()| {
            let state = Arc::new(Mutex::new(GrpcClientState::default()));
            let client_obj = lua.create_table()?;

            // load(paths, file) -> true | (nil, err)
            let load_fn = {
                let state = state.clone();
                let script_path = script_path.clone();
                lua.create_function(move |_lua, (_this, paths, file): (Table, Table, String)| {
                    let script_path = script_path.as_deref();

                    let mut include_paths: Vec<PathBuf> = Vec::new();
                    for v in paths.sequence_values::<String>() {
                        let p = v?;
                        include_paths.push(resolve_path(script_path, &p));
                    }

                    let proto_file = resolve_path(script_path, &file);
                    let schema =
                        wrkr_core::ProtoSchema::compile_from_proto(&proto_file, &include_paths)
                            .map_err(mlua::Error::external)?;

                    let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
                    guard.schema = Some(schema);
                    Ok(true)
                })?
            };

            // connect(target, opts?) -> true | (nil, err)
            let connect_fn = {
                let state = state.clone();
                lua.create_async_function(
                    move |lua, (_this, target, opts): (Table, String, Option<Table>)| {
                        let state = state.clone();
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

                            let client = wrkr_core::GrpcClient::connect(&target, options).await;

                            match client {
                                Ok(client) => {
                                    let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
                                    guard.client = Some(client);
                                    Ok(mlua::MultiValue::from_vec(vec![Value::Boolean(true)]))
                                }
                                Err(err) => Ok(mlua::MultiValue::from_vec(vec![
                                    Value::Nil,
                                    Value::String(lua.create_string(err.to_string().as_bytes())?),
                                ])),
                            }
                        }
                    },
                )?
            };

            // invoke(full_method, req_tbl, opts?) -> res_tbl (never throws on runtime errors)
            let invoke_fn = {
                let state = state.clone();
                let stats = stats.clone();
                lua.create_async_function(
                    move |lua,
                          (_this, full_method, req, opts): (
                        Table,
                        String,
                        Value,
                        Option<Table>,
                    )| {
                        let state = state.clone();
                        let stats = stats.clone();
                        async move {
                            let started = std::time::Instant::now();
                            let (schema, client) = {
                                let guard = state.lock().unwrap_or_else(|p| p.into_inner());
                                (guard.schema.clone(), guard.client.clone())
                            };

                            let t = lua.create_table()?;

                            let Some(schema) = schema else {
                                t.set("ok", false)?;
                                t.set("status", Value::Nil)?;
                                t.set("error_kind", "not_loaded")?;
                                t.set("error", "grpc client: call load() first")?;
                                return Ok(t);
                            };
                            let Some(client) = client else {
                                t.set("ok", false)?;
                                t.set("status", Value::Nil)?;
                                t.set("error_kind", "not_connected")?;
                                t.set("error", "grpc client: call connect() first")?;
                                return Ok(t);
                            };

                            let method =
                                schema.method(&full_method).map_err(mlua::Error::external)?;

                            let mut tags: Vec<(String, String)> = Vec::new();
                            let mut name: Option<String> = None;
                            let mut timeout: Option<Duration> = None;
                            let mut metadata: Vec<(String, String)> = Vec::new();

                            if let Some(opts) = opts {
                                tags = parse_tags(&opts).map_err(mlua::Error::external)?;
                                name = opts.get::<String>("name").ok();
                                if let Ok(timeout_str) = opts.get::<String>("timeout") {
                                    timeout = Some(parse_duration(&timeout_str)?);
                                }
                                metadata = parse_metadata(&opts).map_err(mlua::Error::external)?;
                            }

                            let metric_name = name.clone().unwrap_or_else(|| full_method.clone());

                            if let Some(group) = group_api::current_group(&lua)
                                && !tags.iter().any(|(k, _)| k == "group")
                            {
                                tags.push(("group".to_string(), group));
                            }

                            let req_value = match lua_to_value(&lua, req, Int64Repr::String) {
                                Ok(v) => v,
                                Err(err) => {
                                    t.set("ok", false)?;
                                    t.set("status", Value::Nil)?;
                                    t.set("error_kind", "encode")?;
                                    t.set("error", err.to_string())?;
                                    return Ok(t);
                                }
                            };

                            let res = client
                                .unary(
                                    &method,
                                    req_value,
                                    wrkr_core::GrpcInvokeOptions { timeout, metadata },
                                )
                                .await;

                            match res {
                                Ok(res) => {
                                    stats.record_grpc_request(
                                        wrkr_core::runner::GrpcRequestMeta {
                                            method: wrkr_core::runner::GrpcCallKind::Unary,
                                            name: &metric_name,
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

                                    let headers_tbl = lua.create_table()?;
                                    for (k, v) in res.headers {
                                        headers_tbl.set(k, v)?;
                                    }
                                    t.set("headers", headers_tbl)?;

                                    let trailers_tbl = lua.create_table()?;
                                    for (k, v) in res.trailers {
                                        trailers_tbl.set(k, v)?;
                                    }
                                    t.set("trailers", trailers_tbl)?;

                                    if let Some(msg) = res.response {
                                        let resp = value_to_lua(&lua, &msg, Int64Repr::String)
                                            .map_err(mlua::Error::external)?;
                                        t.set("response", resp)?;
                                    }

                                    Ok(t)
                                }
                                Err(err) => {
                                    let kind = grpc_error_kind(&err);

                                    // best-effort metrics on transport errors
                                    stats.record_grpc_request(
                                        wrkr_core::runner::GrpcRequestMeta {
                                            method: wrkr_core::runner::GrpcCallKind::Unary,
                                            name: &metric_name,
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

            client_obj.set("load", load_fn)?;
            client_obj.set("connect", connect_fn)?;
            client_obj.set("invoke", invoke_fn)?;

            Ok::<_, mlua::Error>(client_obj)
        })?
    };

    client_tbl.set("new", new_fn)?;
    grpc_tbl.set("Client", client_tbl)?;

    Ok(grpc_tbl)
}
