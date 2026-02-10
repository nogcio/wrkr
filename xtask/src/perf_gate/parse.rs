use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

pub fn parse_duration_seconds(s: &str) -> Result<f64> {
    let raw = s.trim();
    if raw.is_empty() {
        bail!("duration is empty");
    }

    let (num, unit) = split_num_unit(raw);
    let amount: f64 = num
        .parse()
        .with_context(|| format!("invalid duration number: {num:?}"))?;
    if amount < 0.0 {
        bail!("duration must be non-negative");
    }

    let seconds = match unit {
        "ms" => amount / 1000.0,
        "s" => amount,
        "m" => amount * 60.0,
        _ => bail!("unsupported duration unit {unit:?} (expected ms|s|m)"),
    };
    Ok(seconds)
}

fn split_num_unit(raw: &str) -> (&str, &str) {
    let mut idx = raw.len();
    for (i, ch) in raw.char_indices() {
        if !(ch.is_ascii_digit() || ch == '.') {
            idx = i;
            break;
        }
    }

    let (num, unit) = raw.split_at(idx);
    (num.trim(), unit.trim())
}

#[derive(Debug)]
pub struct WrkParsed {
    pub rps: f64,
    pub errors: Vec<String>,
}

pub fn parse_wrk(stdout: &str) -> Result<WrkParsed> {
    let mut rps: Option<f64> = None;
    let mut errors: Vec<String> = Vec::new();

    for line in stdout.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("Requests/sec:") {
            let token = rest.split_whitespace().next().unwrap_or("");
            if token.is_empty() {
                bail!("wrk: missing token after Requests/sec:");
            }
            let v: f64 = token
                .parse()
                .with_context(|| format!("wrk: invalid rps token {token:?}"))?;
            rps = Some(v);
        }

        if let Some(rest) = line.strip_prefix("Non-2xx or 3xx responses:") {
            let token = rest.split_whitespace().next().unwrap_or("");
            if let Ok(n) = token.parse::<u64>()
                && n > 0
            {
                errors.push(format!("wrk non-2xx/3xx responses: {n}"));
            }
        }

        if let Some(rest) = line.strip_prefix("Socket errors:") {
            // connect 0, read 12, write 0, timeout 0
            for part in rest.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                let mut it = p.split_whitespace();
                let kind = it.next().unwrap_or("");
                let n_s = it.next().unwrap_or("");
                if kind.is_empty() || n_s.is_empty() {
                    continue;
                }
                if let Ok(n) = n_s.parse::<u64>()
                    && n > 0
                {
                    errors.push(format!("wrk socket {kind}: {n}"));
                }
            }
        }
    }

    let rps = rps.ok_or_else(|| anyhow!("wrk: failed to parse Requests/sec"))?;

    if rps < 0.0 {
        bail!("wrk: rps must be non-negative");
    }

    Ok(WrkParsed { rps, errors })
}

pub fn parse_k6_http_rps(stdout: &str, stderr: &str) -> Result<f64> {
    parse_k6_http_rps_text(stdout)
        .or_else(|| parse_k6_http_rps_text(stderr))
        .ok_or_else(|| anyhow!("k6: failed to parse http rps"))
}

pub fn parse_k6_grpc_rps(stdout: &str, stderr: &str) -> Result<f64> {
    parse_k6_grpc_rps_text(stdout)
        .or_else(|| parse_k6_grpc_rps_text(stderr))
        .ok_or_else(|| anyhow!("k6: failed to parse grpc rps"))
}

fn parse_k6_grpc_rps_text(text: &str) -> Option<f64> {
    for line in text.lines() {
        if !(line.contains("grpc_reqs") || line.contains("iterations")) {
            continue;
        }
        if let Some(v) = parse_slash_s_token(line) {
            return Some(v);
        }
    }

    // fallback: some k6 builds emit only http_reqs
    parse_k6_http_rps_text(text)
}

fn parse_k6_http_rps_text(text: &str) -> Option<f64> {
    for line in text.lines() {
        if line.contains("http_reqs")
            && let Some(v) = parse_slash_s_token(line)
        {
            return Some(v);
        }
    }
    for line in text.lines() {
        if line.contains("iterations")
            && let Some(v) = parse_slash_s_token(line)
        {
            return Some(v);
        }
    }
    for line in text.lines() {
        if let Some(v) = parse_k6_progress_rps(line) {
            return Some(v);
        }
    }
    None
}

fn parse_k6_progress_rps(line: &str) -> Option<f64> {
    // running (02.0s), ... 155325 complete ... iterations
    let s = line.trim();
    if !s.starts_with("running (") {
        return None;
    }
    if !s.contains(" complete") || !s.contains("iterations") {
        return None;
    }

    let seconds = parse_k6_running_seconds(s)?;
    if seconds <= 0.0 {
        return None;
    }
    let completed = parse_k6_completed_iterations(s)?;
    Some((completed as f64) / seconds)
}

fn parse_k6_running_seconds(line: &str) -> Option<f64> {
    // running (02.0s)
    let start = line.find("running (")?;
    let rest = &line[start + "running (".len()..];
    let end = rest.find(')')?;
    let inside = rest[..end].trim();

    let inside = inside.strip_suffix('s')?;
    inside.parse::<f64>().ok()
}

fn parse_k6_completed_iterations(line: &str) -> Option<u64> {
    // ... 155325 complete and ... iterations
    let idx = line.find(" complete")?;
    let prefix = &line[..idx];
    let token = prefix.split_whitespace().last()?;
    token.parse::<u64>().ok()
}

fn parse_slash_s_token(line: &str) -> Option<f64> {
    for raw in line.split_whitespace() {
        let token = raw.trim_matches(|c: char| c == '(' || c == ')' || c == ',');
        if !token.ends_with("/s") {
            continue;
        }
        let number = token.trim_end_matches("/s");
        if let Some(v) = parse_si_float(number) {
            return Some(v);
        }
    }
    None
}

fn parse_si_float(token: &str) -> Option<f64> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    let (num, mul) = match t.chars().last()? {
        'k' | 'K' => (&t[..t.len() - 1], 1_000.0),
        'm' | 'M' => (&t[..t.len() - 1], 1_000_000.0),
        'g' | 'G' => (&t[..t.len() - 1], 1_000_000_000.0),
        _ => (t, 1.0),
    };
    num.parse::<f64>().ok().map(|v| v * mul)
}

pub fn parse_k6_req_failed_rate(metric: &str, stdout: &str, stderr: &str) -> Option<f64> {
    parse_k6_req_failed_rate_text(metric, stdout)
        .or_else(|| parse_k6_req_failed_rate_text(metric, stderr))
}

fn parse_k6_req_failed_rate_text(metric: &str, text: &str) -> Option<f64> {
    for line in text.lines().map(str::trim) {
        if !line.contains(metric) {
            continue;
        }
        // Find first "NN%".
        let mut chars = line.chars().peekable();
        while let Some(ch) = chars.peek().copied() {
            if ch.is_ascii_digit() {
                break;
            }
            chars.next();
        }
        let mut num = String::new();
        while let Some(ch) = chars.peek().copied() {
            if ch.is_ascii_digit() || ch == '.' {
                num.push(ch);
                chars.next();
            } else {
                break;
            }
        }
        if chars.peek().copied() != Some('%') {
            continue;
        }
        let pct: f64 = num.parse().ok()?;
        if pct < 0.0 {
            return None;
        }
        return Some(pct / 100.0);
    }
    None
}

pub fn count_k6_request_failed_warnings(stdout: &str, stderr: &str) -> u32 {
    let needle = r#"msg=\"Request Failed\""#;
    count_substring(stdout, needle) + count_substring(stderr, needle)
}

fn count_substring(text: &str, needle: &str) -> u32 {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut rest = text;
    while let Some(idx) = rest.find(needle) {
        count += 1;
        rest = &rest[idx + needle.len()..];
    }
    count
}

pub fn parse_wrkr_rps(stdout: &str, stderr: &str, duration_seconds: Option<f64>) -> Result<f64> {
    if let Some(v) = try_parse_wrkr_json_metrics(stdout, duration_seconds)? {
        return Ok(v);
    }
    if let Some(v) = try_parse_wrkr_json_metrics(stderr, duration_seconds)? {
        return Ok(v);
    }

    // Legacy fallback: rps: 1234
    for line in stdout.lines().chain(stderr.lines()).map(str::trim) {
        if let Some(rest) = line.strip_prefix("rps:") {
            let token = rest.split_whitespace().next().unwrap_or("");
            if token.is_empty() {
                continue;
            }
            let v: f64 = token.parse().ok().context("invalid wrkr rps token")?;
            if v < 0.0 {
                bail!("wrkr: rps must be non-negative");
            }
            return Ok(v);
        }
    }

    bail!("failed to parse wrkr rps from output")
}

fn try_parse_wrkr_json_metrics(text: &str, duration_seconds: Option<f64>) -> Result<Option<f64>> {
    let mut last_progress: Option<Value> = None;
    let mut last_summary: Option<Value> = None;

    for raw in text.lines() {
        let s = raw.trim();
        if !s.starts_with('{') {
            continue;
        }

        let de = serde_json::Deserializer::from_str(s);
        for v in de.into_iter::<Value>().flatten() {
            if !v.is_object() {
                continue;
            }
            let kind = v.get("kind").and_then(Value::as_str);
            if kind == Some("summary") {
                last_summary = Some(v);
            } else if kind == Some("progress")
                || (kind.is_none() && v.get("elapsed_secs").is_some())
                || (kind.is_none() && v.get("elapsedSeconds").is_some())
            {
                last_progress = Some(v);
            }
        }
    }

    if let Some(summary) = last_summary
        && let Some(dur) = duration_seconds
    {
        if dur <= 0.0 {
            bail!("duration_seconds must be > 0");
        }

        let total = summary
            .pointer("/totals/requestsTotal")
            .and_then(Value::as_u64)
            .or_else(|| summary.get("requestsTotal").and_then(Value::as_u64))
            .ok_or_else(|| anyhow!("wrkr json: missing totals.requestsTotal"))?;

        return Ok(Some((total as f64) / dur));
    }

    let Some(progress) = last_progress else {
        return Ok(None);
    };

    // v1: elapsedSeconds + metrics.totalRequests
    let schema = progress.get("schema").and_then(Value::as_str);
    if schema == Some("wrkr.ndjson.v1") {
        let elapsed = progress
            .get("elapsedSeconds")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let metrics = progress
            .get("metrics")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow!("wrkr json: missing metrics"))?;
        let total = metrics
            .get("totalRequests")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("wrkr json: missing metrics.totalRequests"))?;

        if let Some(dur) = duration_seconds {
            if dur <= 0.0 {
                bail!("duration_seconds must be > 0");
            }
            return Ok(Some((total as f64) / dur));
        }

        if elapsed > 0.0 {
            return Ok(Some((total as f64) / elapsed));
        }
        let rps = metrics
            .get("reqPerSecAvg")
            .and_then(Value::as_f64)
            .or_else(|| metrics.get("requestsPerSec").and_then(Value::as_f64))
            .unwrap_or(0.0);
        return Ok(Some(rps));
    }

    // Legacy format: elapsed_secs + total_requests
    let elapsed = progress.get("elapsed_secs").and_then(Value::as_f64);
    let total = progress.get("total_requests").and_then(Value::as_u64);
    if let (Some(elapsed), Some(total)) = (elapsed, total) {
        if let Some(dur) = duration_seconds {
            if dur <= 0.0 {
                bail!("duration_seconds must be > 0");
            }
            return Ok(Some((total as f64) / dur));
        }
        if elapsed > 0.0 {
            return Ok(Some((total as f64) / elapsed));
        }
    }

    // As last resort, try to read rps fields directly
    let rps = progress
        .get("req_per_sec_avg")
        .and_then(Value::as_f64)
        .or_else(|| progress.get("requests_per_sec").and_then(Value::as_f64));

    Ok(rps)
}
