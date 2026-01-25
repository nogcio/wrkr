use mlua::{Lua, Table, Value};

use crate::value_util::{Int64Repr, value_to_lua};

pub(super) struct InvokeLuaResult {
    pub(super) ok: bool,
    pub(super) status: Option<u16>,
    pub(super) message: Option<String>,
    pub(super) error_kind: Option<String>,
    pub(super) error: Option<String>,
    pub(super) response: Option<wrkr_value::Value>,
}

impl InvokeLuaResult {
    pub(super) fn not_connected() -> Self {
        Self {
            ok: false,
            status: None,
            message: None,
            error_kind: Some("not_connected".to_string()),
            error: Some("grpc client: call connect() first".to_string()),
            response: None,
        }
    }

    pub(super) fn invalid_method() -> Self {
        Self {
            ok: false,
            status: None,
            message: None,
            error_kind: Some("invalid_method".to_string()),
            error: Some("grpc client: method name must be utf-8".to_string()),
            response: None,
        }
    }

    pub(super) fn not_loaded() -> Self {
        Self {
            ok: false,
            status: None,
            message: None,
            error_kind: Some("not_loaded".to_string()),
            error: Some("grpc client: call load() first".to_string()),
            response: None,
        }
    }

    pub(super) fn encode_error(err: String) -> Self {
        Self {
            ok: false,
            status: None,
            message: None,
            error_kind: Some("encode".to_string()),
            error: Some(err),
            response: None,
        }
    }

    pub(super) fn transport_error(kind: wrkr_grpc::GrpcTransportErrorKind, err: String) -> Self {
        Self {
            ok: false,
            status: None,
            message: None,
            error_kind: Some(kind.to_string()),
            error: Some(err),
            response: None,
        }
    }

    pub(super) fn from_unary_result(res: wrkr_grpc::UnaryResult) -> Self {
        Self {
            ok: res.ok,
            status: res.status,
            message: res.message,
            error_kind: res.transport_error_kind.map(|k| k.to_string()),
            error: res.error,
            response: Some(res.response),
        }
    }

    pub(super) fn into_lua_table(self, lua: &Lua, int64_repr: Int64Repr) -> mlua::Result<Table> {
        let t = lua.create_table()?;

        t.set("ok", self.ok)?;
        if let Some(status) = self.status {
            t.set("status", status)?;
        } else {
            t.set("status", Value::Nil)?;
        }

        if let Some(message) = self.message {
            t.set("message", message)?;
        }
        if let Some(error_kind) = self.error_kind {
            t.set("error_kind", error_kind)?;
        }
        if let Some(error) = self.error {
            t.set("error", error)?;
        }

        if let Some(response) = self.response {
            let resp = value_to_lua(lua, &response, int64_repr).map_err(mlua::Error::external)?;
            t.set("response", resp)?;
        }

        Ok(t)
    }
}
