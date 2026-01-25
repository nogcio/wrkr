use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use mlua::{Lua, Table, Value};

use crate::Result;

use super::opts::{ClientNewLuaOptions, ConnectLuaOptions, InvokeLuaOptions};
use super::path::resolve_path;
use super::result::InvokeLuaResult;

fn grpc_error_kind(err: &wrkr_grpc::Error) -> wrkr_grpc::GrpcTransportErrorKind {
    err.transport_error_kind()
}

pub(super) fn create_client_table(
    lua: &Lua,
    run_ctx: Arc<wrkr_core::RunScenariosContext>,
    metrics_ctx: wrkr_core::MetricsContext,
    script_path: &Path,
    max_vus: u64,
) -> Result<Table> {
    use crate::value_util::{Int64Repr, lua_to_value};

    let client_tbl = lua.create_table()?;

    let metrics = run_ctx.metrics.clone();
    let request_metrics = run_ctx.request_metrics;
    let grpc_registry = run_ctx.grpc.clone();

    let new_fn = {
        let script_path = script_path.to_path_buf();
        let metrics = metrics.clone();
        let metrics_ctx = metrics_ctx.clone();
        let grpc_registry = grpc_registry.clone();
        lua.create_function(move |lua, opts: Option<Table>| {
            let mut pool_size = wrkr_grpc::shared::default_pool_size(max_vus);
            let parsed = ClientNewLuaOptions::parse(opts, max_vus)?;
            if let Some(p) = parsed.pool_size {
                pool_size = p;
            }

            let shared = grpc_registry.get_or_create(pool_size);
            let client_obj = lua.create_table()?;

            // load(paths, file) -> true | (nil, err)
            let load_fn = {
                let shared = shared.clone();
                let script_path = script_path.clone();
                lua.create_function(move |_lua, (_this, paths, file): (Table, Table, String)| {
                    let mut include_paths: Vec<PathBuf> = Vec::new();
                    for v in paths.sequence_values::<String>() {
                        let p = v?;
                        include_paths.push(resolve_path(&script_path, &p));
                    }

                    let proto_file = resolve_path(&script_path, &file);
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
                            let options = ConnectLuaOptions::parse(opts)?.into_connect_options();

                            match shared.connect(target, options).await {
                                Ok(()) => {
                                    Ok(mlua::MultiValue::from_vec(vec![Value::Boolean(true)]))
                                }
                                Err(err) => {
                                    let msg = err.to_string();
                                    Ok(mlua::MultiValue::from_vec(vec![
                                        Value::Nil,
                                        Value::String(lua.create_string(msg.as_bytes())?),
                                    ]))
                                }
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
                let metrics = metrics.clone();
                let metrics_ctx = metrics_ctx.clone();
                lua.create_async_function(
                    move |lua,
                          (_this, full_method, req, opts): (
                        Table,
                        mlua::String,
                        Value,
                        Option<Table>,
                    )| {
                        let shared = shared.clone();
                        let metrics = metrics.clone();
                        let metrics_ctx = metrics_ctx.clone();
                        async move {
                            let client = shared.client();

                            let Some(client) = client else {
                                return InvokeLuaResult::not_connected()
                                    .into_lua_table(&lua, Int64Repr::Integer);
                            };

                            let full_method = match full_method.to_str() {
                                Ok(s) => s,
                                Err(_) => {
                                    return InvokeLuaResult::invalid_method()
                                        .into_lua_table(&lua, Int64Repr::Integer);
                                }
                            };

                            let full_method_str: &str = full_method.as_ref();

                            let method = match shared.method(full_method_str) {
                                Ok(m) => m,
                                Err(_) => {
                                    return InvokeLuaResult::not_loaded()
                                        .into_lua_table(&lua, Int64Repr::Integer);
                                }
                            };

                            let mut _tags: Vec<(String, String)> = Vec::new();
                            let parsed =
                                InvokeLuaOptions::parse(opts).map_err(mlua::Error::external)?;
                            _tags = parsed.tags;
                            let timeout = parsed.timeout;
                            let metadata = parsed.metadata;
                            let int64_repr = parsed.int64_repr;

                            metrics_ctx.merge_scenario_tags_if_missing(
                                &mut _tags,
                                &["scenario", "protocol", "error_kind", "group"],
                            );

                            if let Some(group) = super::super::group::current_group(&lua)
                                && !_tags.iter().any(|(k, _)| k == "group")
                            {
                                _tags.push(("group".to_string(), group));
                            }

                            let extra_tags: Vec<(&str, &str)> = _tags
                                .iter()
                                .map(|(k, v)| (k.as_str(), v.as_str()))
                                .collect();

                            let invoke_opts = wrkr_grpc::InvokeOptions { timeout, metadata };

                            // Always encode to bytes here so we can account bytes_sent without
                            // double-encoding inside the client.
                            let req_bytes = match req {
                                Value::String(req_bytes) => {
                                    bytes::Bytes::copy_from_slice(req_bytes.as_bytes().as_ref())
                                }
                                other => {
                                    let req_value =
                                        match lua_to_value(&lua, other, Int64Repr::String) {
                                            Ok(v) => v,
                                            Err(err) => {
                                                return InvokeLuaResult::encode_error(
                                                    err.to_string(),
                                                )
                                                .into_lua_table(&lua, Int64Repr::Integer);
                                            }
                                        };

                                    match wrkr_grpc::encode_unary_request(
                                        method.as_ref(),
                                        &req_value,
                                    ) {
                                        Ok(bytes) => bytes,
                                        Err(err) => {
                                            return InvokeLuaResult::encode_error(err.to_string())
                                                .into_lua_table(&lua, Int64Repr::Integer);
                                        }
                                    }
                                }
                            };

                            let started = Instant::now();
                            let res = client
                                .unary_bytes(method.as_ref(), req_bytes.clone(), invoke_opts)
                                .await;
                            let elapsed = started.elapsed();

                            match res {
                                Ok(res) => {
                                    // Transport succeeded (even if gRPC status is non-OK).
                                    request_metrics.record_request(
                                        &metrics,
                                        wrkr_core::RequestSample {
                                            scenario: metrics_ctx.scenario(),
                                            protocol: wrkr_core::Protocol::Grpc,
                                            ok: true,
                                            latency: elapsed,
                                            bytes_received: res.bytes_received,
                                            bytes_sent: res.bytes_sent,
                                            error_kind: None,
                                        },
                                        &extra_tags,
                                    );

                                    InvokeLuaResult::from_unary_result(res)
                                        .into_lua_table(&lua, int64_repr)
                                }
                                Err(err) => {
                                    let kind = grpc_error_kind(&err);
                                    let kind_s = kind.to_string();

                                    request_metrics.record_request(
                                        &metrics,
                                        wrkr_core::RequestSample {
                                            scenario: metrics_ctx.scenario(),
                                            protocol: wrkr_core::Protocol::Grpc,
                                            ok: false,
                                            latency: elapsed,
                                            bytes_received: 0,
                                            bytes_sent: req_bytes.len() as u64,
                                            error_kind: Some(kind_s.as_str()),
                                        },
                                        &extra_tags,
                                    );

                                    InvokeLuaResult::transport_error(kind, err.to_string())
                                        .into_lua_table(&lua, Int64Repr::Integer)
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

                        match wrkr_grpc::encode_unary_request(method.as_ref(), &req_value) {
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

    Ok(client_tbl)
}
