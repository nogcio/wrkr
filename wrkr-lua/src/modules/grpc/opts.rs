use std::time::Duration;

use mlua::{Table, Value};

use crate::Result;

use crate::value_util::Int64Repr;

pub(super) struct ClientNewLuaOptions {
    pub(super) pool_size: Option<usize>,
}

impl ClientNewLuaOptions {
    pub(super) fn parse(opts: Option<Table>, max_vus: u64) -> mlua::Result<Self> {
        let Some(opts) = opts else {
            return Ok(Self { pool_size: None });
        };

        Ok(Self {
            pool_size: parse_pool_size(&opts, max_vus)?,
        })
    }
}

pub(super) struct ConnectLuaOptions {
    pub(super) timeout: Option<Duration>,
    pub(super) tls: Option<TlsLuaOptions>,
}

impl ConnectLuaOptions {
    pub(super) fn parse(opts: Option<Table>) -> mlua::Result<Self> {
        let Some(opts) = opts else {
            return Ok(Self {
                timeout: None,
                tls: None,
            });
        };

        let timeout = match opts.get::<Option<String>>("timeout")? {
            Some(v) => Some(parse_duration(&v)?),
            None => None,
        };

        let tls = match opts.get::<Option<Table>>("tls")? {
            Some(t) => Some(TlsLuaOptions::parse(&t)?),
            None => None,
        };

        Ok(Self { timeout, tls })
    }

    pub(super) fn into_connect_options(self) -> wrkr_grpc::ConnectOptions {
        wrkr_grpc::ConnectOptions {
            timeout: self.timeout,
            tls: self.tls.map(TlsLuaOptions::into_tls_config),
        }
    }
}

pub(super) struct TlsLuaOptions {
    pub(super) server_name: Option<String>,
    pub(super) insecure_skip_verify: Option<bool>,
    pub(super) ca_pem: Option<Vec<u8>>,
    pub(super) cert_pem: Option<Vec<u8>>,
    pub(super) key_pem: Option<Vec<u8>>,
}

impl TlsLuaOptions {
    fn parse(tls_tbl: &Table) -> mlua::Result<Self> {
        let server_name = tls_tbl.get::<Option<String>>("server_name")?;
        let insecure_skip_verify = tls_tbl.get::<Option<bool>>("insecure_skip_verify")?;
        let ca_pem = tls_tbl
            .get::<Option<mlua::String>>("ca")?
            .map(|s| s.as_bytes().to_vec());
        let cert_pem = tls_tbl
            .get::<Option<mlua::String>>("cert")?
            .map(|s| s.as_bytes().to_vec());
        let key_pem = tls_tbl
            .get::<Option<mlua::String>>("key")?
            .map(|s| s.as_bytes().to_vec());

        Ok(Self {
            server_name,
            insecure_skip_verify,
            ca_pem,
            cert_pem,
            key_pem,
        })
    }

    fn into_tls_config(self) -> wrkr_grpc::TlsConfig {
        wrkr_grpc::TlsConfig {
            domain_name: self.server_name,
            insecure_skip_verify: self.insecure_skip_verify.unwrap_or(false),
            ca_pem: self.ca_pem,
            identity_pem: self.cert_pem,
            identity_key_pem: self.key_pem,
        }
    }
}

pub(super) struct InvokeLuaOptions {
    pub(super) tags: Vec<(String, String)>,
    pub(super) timeout: Option<Duration>,
    pub(super) metadata: Vec<(String, String)>,
    pub(super) int64_repr: Int64Repr,
}

impl InvokeLuaOptions {
    pub(super) fn parse(opts: Option<Table>) -> mlua::Result<Self> {
        let Some(opts) = opts else {
            return Ok(Self {
                tags: Vec::new(),
                timeout: None,
                metadata: Vec::new(),
                int64_repr: Int64Repr::Integer,
            });
        };

        let tags = parse_tags(&opts).map_err(mlua::Error::external)?;

        let timeout = match opts.get::<Option<String>>("timeout")? {
            Some(v) => Some(parse_duration(&v)?),
            None => None,
        };

        let metadata = parse_metadata(&opts).map_err(mlua::Error::external)?;

        let int64_repr = match opts.get::<Option<String>>("int64")? {
            Some(int64_str) => match int64_str.as_str() {
                "integer" => Int64Repr::Integer,
                "string" => Int64Repr::String,
                _ => {
                    return Err(mlua::Error::external(
                        "grpc invoke opts.int64 must be 'integer' or 'string'",
                    ));
                }
            },
            None => Int64Repr::Integer,
        };

        Ok(Self {
            tags,
            timeout,
            metadata,
            int64_repr,
        })
    }
}

fn parse_pool_size(opts: &Table, max_vus: u64) -> mlua::Result<Option<usize>> {
    let Some(pool_val) = opts.get::<Option<Value>>("pool_size")? else {
        return Ok(None);
    };

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
        mlua::Error::external("grpc.Client.new: pool_size is too large for this platform")
    })?;

    let max_pool = (max_vus as usize).clamp(1, 1024);
    if requested > max_pool {
        return Err(mlua::Error::external(format!(
            "grpc.Client.new: pool_size must be between 1 and {max_pool}",
        )));
    }

    Ok(Some(requested))
}

pub(super) fn parse_duration(v: &str) -> mlua::Result<Duration> {
    humantime::parse_duration(v)
        .map_err(|e| mlua::Error::external(format!("invalid duration '{v}': {e}")))
}

pub(super) fn parse_metadata(opts: &Table) -> Result<Vec<(String, String)>> {
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

pub(super) fn parse_tags(opts: &Table) -> Result<Vec<(String, String)>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_metadata_supports_multi_values() {
        let lua = mlua::Lua::new();
        let opts = lua
            .create_table()
            .unwrap_or_else(|err| panic!("create_table: {err}"));
        let md = lua
            .create_table()
            .unwrap_or_else(|err| panic!("create_table: {err}"));
        let arr = lua
            .create_table()
            .unwrap_or_else(|err| panic!("create_table: {err}"));
        arr.set(1, "a")
            .unwrap_or_else(|err| panic!("set arr[1]: {err}"));
        arr.set(2, "b")
            .unwrap_or_else(|err| panic!("set arr[2]: {err}"));
        md.set("x", arr)
            .unwrap_or_else(|err| panic!("set md.x: {err}"));
        opts.set("metadata", md)
            .unwrap_or_else(|err| panic!("set opts.metadata: {err}"));

        let out = parse_metadata(&opts).unwrap_or_else(|err| panic!("parse_metadata: {err}"));
        assert_eq!(
            out,
            vec![
                ("x".to_string(), "a".to_string()),
                ("x".to_string(), "b".to_string())
            ]
        );
    }
}
