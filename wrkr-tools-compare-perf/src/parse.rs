use crate::types::{Rps, RunResult};
use anyhow::{Context, Result, bail};

pub(crate) fn parse_wrk_rps(stdout: &str) -> Result<Rps> {
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Requests/sec:") {
            let v = rest.split_whitespace().next().context("wrk rps token")?;
            let rps: f64 = v.parse().context("parse wrk rps")?;
            return Ok(Rps(rps));
        }
    }
    bail!("failed to parse wrk RPS")
}

pub(crate) fn parse_wrkr_rps(res: &RunResult) -> Result<Rps> {
    if let Ok(rps) = parse_wrkr_rps_text(&res.stdout) {
        return Ok(rps);
    }
    parse_wrkr_rps_text(&res.stderr).with_context(|| wrkr_parse_diag(res))
}

fn parse_wrkr_rps_text(text: &str) -> Result<Rps> {
    // Legacy: rps: 1234
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("rps:") {
            let v = rest.split_whitespace().next().context("wrkr rps token")?;
            let rps: f64 = v.parse().context("parse wrkr rps")?;
            return Ok(Rps(rps));
        }
    }

    // k6-like summary: prefer grpc_reqs if present (gRPC scripts also print http_reqs=0).
    let mut grpc_rps: Option<f64> = None;
    let mut http_rps: Option<f64> = None;
    let mut iterations_rps: Option<f64> = None;

    for line in text.lines() {
        if line.contains("grpc_reqs") {
            if let Some(rps) = parse_paren_rate_token(line) {
                grpc_rps = Some(rps);
            }
        } else if line.contains("http_reqs") {
            if let Some(rps) = parse_paren_rate_token(line) {
                http_rps = Some(rps);
            }
        } else if line.contains("iterations")
            && let Some(rps) = parse_paren_rate_token(line)
        {
            iterations_rps = Some(rps);
        }
    }

    if let Some(rps) = grpc_rps {
        return Ok(Rps(rps));
    }
    if let Some(rps) = http_rps {
        return Ok(Rps(rps));
    }
    if let Some(rps) = iterations_rps {
        return Ok(Rps(rps));
    }

    bail!("failed to parse wrkr RPS")
}

fn wrkr_parse_diag(res: &RunResult) -> String {
    let out = truncate(&tail_lines(&res.stdout, 12), 1200);
    let err = truncate(&tail_lines(&res.stderr, 12), 1200);
    format!(
        "failed to parse wrkr RPS\n--- wrkr stdout (tail) ---\n{out}\n--- wrkr stderr (tail) ---\n{err}"
    )
}

pub(crate) fn parse_k6_http_rps(res: &RunResult) -> Result<Rps> {
    if let Ok(rps) = parse_k6_http_rps_text(&res.stdout) {
        return Ok(rps);
    }
    parse_k6_http_rps_text(&res.stderr).with_context(|| k6_parse_diag("http", res))
}

fn parse_k6_http_rps_text(text: &str) -> Result<Rps> {
    // Preferred: http_reqs...: ... 1234.5/s
    for line in text.lines() {
        if line.contains("http_reqs")
            && let Some(rate) = parse_slash_s_token(line)
        {
            return Ok(Rps(rate));
        }
    }

    // Fallback: our k6 perf scripts are 1 request per iteration.
    for line in text.lines() {
        if line.contains("iterations")
            && let Some(rate) = parse_slash_s_token(line)
        {
            return Ok(Rps(rate));
        }
    }

    // Last resort: parse progress line and compute complete/duration.
    for line in text.lines() {
        if let Some(rate) = parse_k6_progress_rps(line) {
            return Ok(Rps(rate));
        }
    }

    bail!("failed to parse k6 http RPS")
}

pub(crate) fn parse_k6_grpc_rps(res: &RunResult) -> Result<Rps> {
    if let Ok(rps) = parse_k6_grpc_rps_text(&res.stdout) {
        return Ok(rps);
    }
    parse_k6_grpc_rps_text(&res.stderr).with_context(|| k6_parse_diag("grpc", res))
}

fn parse_k6_grpc_rps_text(text: &str) -> Result<Rps> {
    for line in text.lines() {
        if !(line.contains("grpc_reqs") || line.contains("iterations")) {
            continue;
        }
        if let Some(rate) = parse_slash_s_token(line) {
            return Ok(Rps(rate));
        }
    }

    // Fallback: some k6 builds print only http_reqs; try that too.
    parse_k6_http_rps_text(text)
}

fn parse_paren_rate_token(line: &str) -> Option<f64> {
    // Example: http_reqs.......................: 1085653 (217130.60000/s)
    // We want 217130.60000
    let start = line.find('(')?;
    let end = line[start..].find(')')? + start;
    let inside = &line[start + 1..end];
    let inside = inside.trim();
    let inside = inside.strip_suffix("/s")?.trim();
    inside.parse().ok()
}

fn parse_slash_s_token(line: &str) -> Option<f64> {
    // Example tokens: 217130.60/s or (217130.60000/s)
    for raw in line.split_whitespace() {
        let token = raw.trim_matches(|c| c == '(' || c == ')' || c == ',');
        let token = token.strip_suffix("/s")?;
        if let Some(v) = parse_si_float(token) {
            return Some(v);
        }
    }
    None
}

fn parse_si_float(token: &str) -> Option<f64> {
    // Accept plain float ("123.4") and SI suffixes used by some k6 builds ("123.4k", "1.2M").
    let (num, mul) = match token.chars().last()? {
        'k' | 'K' => (&token[..token.len().saturating_sub(1)], 1_000.0),
        'm' | 'M' => (&token[..token.len().saturating_sub(1)], 1_000_000.0),
        'g' | 'G' => (&token[..token.len().saturating_sub(1)], 1_000_000_000.0),
        _ => (token, 1.0),
    };

    let v = num.parse::<f64>().ok()?;
    Some(v * mul)
}

fn parse_k6_progress_rps(line: &str) -> Option<f64> {
    // Example:
    // running (02.0s), 000/256 VUs, 155325 complete and 0 interrupted iterations
    let line = line.trim();
    if !line.starts_with("running (") || !line.contains(" complete") || !line.contains("iterations")
    {
        return None;
    }

    let seconds = parse_k6_running_seconds(line)?;
    if seconds <= 0.0 {
        return None;
    }

    let completed = parse_k6_completed_iterations(line)?;
    Some(completed as f64 / seconds)
}

fn parse_k6_running_seconds(line: &str) -> Option<f64> {
    let rest = line.strip_prefix("running (")?;
    let end = rest.find(')')?;
    let inside = &rest[..end];
    let inside = inside.trim();
    let inside = inside.strip_suffix('s')?;
    inside.parse::<f64>().ok()
}

fn parse_k6_completed_iterations(line: &str) -> Option<u64> {
    // Find the token immediately before "complete".
    let mut prev: Option<&str> = None;
    for tok in line.split_whitespace() {
        if tok == "complete" {
            let n = prev?.trim_end_matches(',').replace(',', "");
            return n.parse::<u64>().ok();
        }
        prev = Some(tok);
    }
    None
}

fn k6_parse_diag(kind: &str, res: &RunResult) -> String {
    let out = truncate(&tail_lines(&res.stdout, 12), 1200);
    let err = truncate(&tail_lines(&res.stderr, 12), 1200);
    format!(
        "failed to parse k6 {kind} RPS\n--- k6 stdout (tail) ---\n{out}\n--- k6 stderr (tail) ---\n{err}"
    )
}

fn tail_lines(s: &str, n: usize) -> String {
    let mut lines: Vec<&str> = s.lines().rev().take(n).collect();
    lines.reverse();
    lines.join("\n")
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }

    let mut out = String::with_capacity(max_chars + 3);
    out.extend(s.chars().take(max_chars));
    out.push_str("...");
    out
}
