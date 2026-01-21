use crate::types::{Mb, Rps, RunResult};

pub(crate) fn print_http_summary(
    wrk: Option<&RunResult>,
    wrkr: &RunResult,
    k6: Option<&RunResult>,
    wrk_rps: Option<Rps>,
    wrkr_rps: Rps,
    k6_rps: Option<Rps>,
) {
    println!("\nmetric                      |          wrk |         wrkr |           k6");
    println!("---------------------------+--------------+--------------+--------------");

    let wrk_rps_s = wrk_rps
        .map(|r| format!("{:.3}", r.0))
        .unwrap_or("-".to_string());
    let k6_rps_s = k6_rps
        .map(|r| format!("{:.3}", r.0))
        .unwrap_or("-".to_string());

    let wrk_mb = wrk
        .map(|r| format!("{:.2}", Mb::from_bytes(r.peak_rss_bytes).0))
        .unwrap_or("-".to_string());
    let k6_mb = k6
        .map(|r| format!("{:.2}", Mb::from_bytes(r.peak_rss_bytes).0))
        .unwrap_or("-".to_string());

    println!(
        "rps                         | {:>12} | {:>12.3} | {:>12}",
        wrk_rps_s, wrkr_rps.0, k6_rps_s
    );
    println!(
        "max_rss_mb                  | {:>12} | {:>12.2} | {:>12}",
        wrk_mb,
        Mb::from_bytes(wrkr.peak_rss_bytes).0,
        k6_mb
    );
}

pub(crate) fn print_grpc_summary(
    wrkr: &RunResult,
    k6: Option<&RunResult>,
    wrkr_rps: Rps,
    k6_rps: Option<Rps>,
) {
    println!("\nmetric                      |          wrk |         wrkr |           k6");
    println!("---------------------------+--------------+--------------+--------------");

    let k6_rps_s = k6_rps
        .map(|r| format!("{:.3}", r.0))
        .unwrap_or("-".to_string());
    let k6_mb = k6
        .map(|r| format!("{:.2}", Mb::from_bytes(r.peak_rss_bytes).0))
        .unwrap_or("-".to_string());

    println!(
        "rps                         | {:>12} | {:>12.3} | {:>12}",
        "-", wrkr_rps.0, k6_rps_s
    );
    println!(
        "max_rss_mb                  | {:>12} | {:>12.2} | {:>12}",
        "-",
        Mb::from_bytes(wrkr.peak_rss_bytes).0,
        k6_mb
    );
}
