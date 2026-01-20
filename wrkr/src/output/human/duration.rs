use std::time::Duration;

pub(crate) fn format_duration_single(d: Duration) -> String {
    // Always render as a single rounded component in one of: us, ms, s.
    // This keeps the output short and consistent for progress lines.

    let total_ns: u128 = (d.as_secs() as u128) * 1_000_000_000u128 + (d.subsec_nanos() as u128);

    const NS_PER_US: u128 = 1_000;
    const NS_PER_MS: u128 = 1_000_000;
    const NS_PER_S: u128 = 1_000_000_000;

    fn round_div(value: u128, unit: u128) -> u128 {
        // Round to nearest integer (ties round up).
        (value + (unit / 2)) / unit
    }

    if total_ns >= NS_PER_S {
        return format!("{}s", round_div(total_ns, NS_PER_S));
    }
    if total_ns >= NS_PER_MS {
        return format!("{}ms", round_div(total_ns, NS_PER_MS));
    }

    // For sub-millisecond durations we still want a stable unit.
    format!("{}us", round_div(total_ns, NS_PER_US))
}
