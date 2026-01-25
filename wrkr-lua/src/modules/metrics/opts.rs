use mlua::{Lua, Table, Value};

pub(super) fn tags_from_lua(tags: Option<Table>) -> mlua::Result<Vec<(String, String)>> {
    let Some(tags) = tags else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for pair in tags.pairs::<Value, Value>() {
        let Ok((k, v)) = pair else {
            continue;
        };

        let k = match k {
            Value::String(s) => s.to_string_lossy().to_string(),
            _ => continue,
        };

        let v = match v {
            Value::String(s) => s.to_string_lossy().to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Number(n) if n.is_finite() => n.to_string(),
            Value::Boolean(b) => b.to_string(),
            _ => continue,
        };

        out.push((k, v));
    }

    Ok(out)
}

pub(super) fn add_group_tag_if_missing(lua: &Lua, tags: &mut Vec<(String, String)>) {
    if tags.iter().any(|(k, _)| k == "group") {
        return;
    }

    if let Some(group) = super::super::group::current_group(lua) {
        tags.push(("group".to_string(), group));
    }
}

pub(super) fn resolve_tags(
    metrics: &wrkr_metrics::Registry,
    tags: &[(String, String)],
) -> wrkr_metrics::TagSet {
    let tag_refs: Vec<(&str, &str)> = tags.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    metrics.resolve_tags(&tag_refs)
}

pub(super) struct MetricAddLuaArgs {
    pub(super) value: Value,
    pub(super) tags: Vec<(String, String)>,
}

impl MetricAddLuaArgs {
    pub(super) fn parse(
        lua: &Lua,
        metrics_ctx: &wrkr_core::MetricsContext,
        value: Value,
        tags: Option<Table>,
    ) -> mlua::Result<Self> {
        let mut tags = tags_from_lua(tags)?;

        // Base metric tags always include scenario + scenario.tags.
        metrics_ctx.merge_base_tags_if_missing(&mut tags, &["group"]);

        add_group_tag_if_missing(lua, &mut tags);
        Ok(Self { value, tags })
    }
}
