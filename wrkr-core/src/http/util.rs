pub(super) fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
}

pub(super) fn host_header_value(parsed: &url::Url) -> Option<String> {
    let host = parsed.host_str()?;
    match parsed.port() {
        Some(port) if port != 80 => Some(format!("{host}:{port}")),
        _ => Some(host.to_string()),
    }
}
