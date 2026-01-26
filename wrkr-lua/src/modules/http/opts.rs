use std::time::Duration;

use mlua::{Table, Value};

#[derive(Debug, Default, Clone)]
pub(super) struct HttpRequestOptions {
    pub(super) headers: Vec<(String, String)>,
    pub(super) params: Vec<(String, String)>,
    pub(super) timeout: Option<Duration>,
    pub(super) tags: Vec<(String, String)>,
    pub(super) name: Option<String>,
}

pub(super) fn parse_http_opts(opts: Option<Table>) -> crate::Result<HttpRequestOptions> {
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
            Some(humantime::parse_duration(&s).map_err(|_| crate::Error::InvalidDuration)?)
        }
        _ => return Err(crate::Error::InvalidDuration),
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
        Some(Value::Integer(i)) => Some(i.to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(_) => None,
    };

    Ok(HttpRequestOptions {
        headers,
        params,
        timeout,
        tags,
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_opts_none_is_defaults() {
        let lua = mlua::Lua::new();
        let out = parse_http_opts(None).unwrap_or_else(|err| panic!("parse_http_opts: {err}"));
        assert!(out.headers.is_empty());
        assert!(out.params.is_empty());
        assert!(out.tags.is_empty());
        assert!(out.timeout.is_none());

        // keep lua alive to avoid dropping issues in debug scenarios
        drop(lua);
    }

    #[test]
    fn parse_http_opts_timeout_string() {
        let lua = mlua::Lua::new();
        let opts = lua
            .create_table()
            .unwrap_or_else(|err| panic!("create_table: {err}"));
        opts.set("timeout", "150ms")
            .unwrap_or_else(|err| panic!("set timeout: {err}"));

        let out =
            parse_http_opts(Some(opts)).unwrap_or_else(|err| panic!("parse_http_opts: {err}"));
        assert_eq!(out.timeout, Some(Duration::from_millis(150)));
    }
}
