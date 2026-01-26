use mlua::{Lua, Table};

pub(super) struct HttpLuaResponse {
    pub(super) status: u16,
    pub(super) body: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) error: Option<String>,
}

impl HttpLuaResponse {
    pub(super) fn ok(res: wrkr_http::HttpResponse) -> Self {
        Self {
            status: res.status,
            body: res.body_utf8().unwrap_or("").to_string(),
            headers: res.headers,
            error: None,
        }
    }

    pub(super) fn err(err: wrkr_http::Error) -> Self {
        Self {
            status: 0,
            body: String::new(),
            headers: Vec::new(),
            error: Some(err.to_string()),
        }
    }

    pub(super) fn into_lua_table(self, lua: &Lua) -> mlua::Result<Table> {
        let t = lua.create_table()?;
        t.set("status", self.status)?;
        t.set("body", self.body)?;

        let headers_tbl = lua.create_table()?;
        for (k, v) in self.headers {
            headers_tbl.set(k, v)?;
        }
        t.set("headers", headers_tbl)?;

        if let Some(error) = self.error {
            t.set("error", error)?;
        }
        Ok(t)
    }
}
