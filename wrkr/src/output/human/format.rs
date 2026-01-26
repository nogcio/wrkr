pub(crate) fn format_bytes(b: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;

    if b >= GIB {
        return format!("{:.2}GiB", (b as f64) / (GIB as f64));
    }
    if b >= MIB {
        return format!("{:.2}MiB", (b as f64) / (MIB as f64));
    }
    if b >= KIB {
        return format!("{:.2}KiB", (b as f64) / (KIB as f64));
    }

    format!("{b}B")
}

pub(crate) fn format_tags_inline(tags: &[(String, String)], exclude: &[&str]) -> String {
    let mut filtered: Vec<(String, String)> = tags
        .iter()
        .filter(|(k, _)| !exclude.iter().any(|e| e == &k.as_str()))
        .cloned()
        .collect();

    filtered.sort_by(|(ak, av), (bk, bv)| ak.cmp(bk).then_with(|| av.cmp(bv)));

    if filtered.is_empty() {
        return String::new();
    }

    let inner = filtered
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ");

    format!("{{{inner}}}")
}

pub(crate) fn format_rate(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.0}")
    } else {
        "0".to_string()
    }
}
