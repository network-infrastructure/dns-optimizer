use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::catalog::{Catalog, DnsServer, GoalProfile};
use crate::changer::get_current_system_dns;
use crate::protocol::{query_doh, query_dot, query_tcp, query_udp, RecordType, ResponseCode, Transport};

pub const DEFAULT_TEST_DOMAINS: &[&str] = &[
    "example.com",
    "cloudflare.com",
    "github.com",
    "wikipedia.org",
    "mozilla.org",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub profile: GoalProfile,
    pub transport: Transport,
    pub runs_per_domain: usize,
    pub timeout_ms: u64,
    pub concurrency: usize,
    pub include_current_dns: bool,
    pub domains: Vec<String>,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            profile: GoalProfile::Speed,
            transport: Transport::Udp,
            runs_per_domain: 2,
            timeout_ms: 2000,
            concurrency: 12,
            include_current_dns: true,
            domains: DEFAULT_TEST_DOMAINS.iter().map(|s| s.to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerBenchmarkResult {
    pub server_id: String,
    pub name: String,
    pub provider: String,
    pub primary_ip: String,
    pub doh: Option<String>,
    pub dot: Option<String>,
    pub transport: Transport,
    pub is_current_system_dns: bool,
    pub min_ms: f64,
    pub median_ms: f64,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub jitter_ms: f64,
    pub success_rate: f64,
    pub queries_sent: usize,
    pub queries_ok: usize,
    pub dnssec: bool,
    pub privacy_score: u8,
    pub blocks_ads: bool,
    pub family_safe: bool,
    pub smart_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub timestamp: DateTime<Utc>,
    pub profile: GoalProfile,
    pub transport: Transport,
    pub results: Vec<ServerBenchmarkResult>,
    pub recommended: Option<ServerBenchmarkResult>,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::INFINITY;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn compute_smart_score(
    median_ms: f64,
    success_rate: f64,
    profile: GoalProfile,
    privacy_score: u8,
    dnssec: bool,
    blocks_ads: bool,
    family_safe: bool,
) -> f64 {
    if success_rate <= 0.0 || !median_ms.is_finite() {
        return 0.0;
    }
    // Speed score normalized 0..100 (lower latency = higher score)
    let speed_score = (100.0 / (1.0 + (median_ms / 40.0))).clamp(0.0, 100.0);
    let reliability = success_rate * 100.0;
    let privacy = (privacy_score as f64 / 5.0) * 100.0 + if dnssec { 10.0 } else { 0.0 };

    match profile {
        GoalProfile::Speed => 0.75 * speed_score + 0.25 * reliability,
        GoalProfile::Privacy => 0.45 * speed_score + 0.20 * reliability + 0.35 * privacy.min(100.0),
        GoalProfile::AdBlock => {
            let bonus = if blocks_ads { 25.0 } else { 0.0 };
            0.55 * speed_score + 0.20 * reliability + bonus
        }
        GoalProfile::Family => {
            let bonus = if family_safe { 25.0 } else { 0.0 };
            0.55 * speed_score + 0.20 * reliability + bonus
        }
    }
}

async fn bench_single_target(
    client: Arc<reqwest::Client>,
    server_id: String,
    name: String,
    provider: String,
    primary_ip: String,
    doh: Option<String>,
    dot: Option<String>,
    is_current: bool,
    dnssec: bool,
    privacy_score: u8,
    blocks_ads: bool,
    family_safe: bool,
    config: &BenchmarkConfig,
) -> ServerBenchmarkResult {
    let timeout_dur = Duration::from_millis(config.timeout_ms);
    let mut latencies = Vec::new();
    let mut sent = 0usize;

    for _run in 0..config.runs_per_domain {
        for domain in &config.domains {
            sent += 1;
            let res = match config.transport {
                Transport::Udp => query_udp(&primary_ip, domain, RecordType::A, timeout_dur).await,
                Transport::Tcp => query_tcp(&primary_ip, domain, RecordType::A, timeout_dur).await,
                Transport::Dot => {
                    if let Some(ref host) = dot {
                        query_dot(&primary_ip, host, domain, RecordType::A, timeout_dur).await
                    } else {
                        continue;
                    }
                }
                Transport::Doh => {
                    if let Some(ref url) = doh {
                        query_doh(&client, url, domain, RecordType::A, timeout_dur).await
                    } else {
                        continue;
                    }
                }
            };

            if let Ok(r) = res {
                if r.rcode == ResponseCode::NoError || r.rcode == ResponseCode::NxDomain {
                    latencies.push(r.latency_ms);
                }
            }
        }
    }

    let ok = latencies.len();
    let success_rate = if sent > 0 {
        ok as f64 / sent as f64
    } else {
        0.0
    };

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let (min_ms, median_ms, avg_ms, p95_ms, jitter_ms) = if latencies.is_empty() {
        (9999.0, 9999.0, 9999.0, 9999.0, 0.0)
    } else {
        let min = latencies[0];
        let med = percentile(&latencies, 0.50);
        let p95 = percentile(&latencies, 0.95);
        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let var = latencies
            .iter()
            .map(|v| (v - avg).powi(2))
            .sum::<f64>()
            / latencies.len() as f64;
        (min, med, avg, p95, var.sqrt())
    };

    let smart_score = compute_smart_score(
        median_ms,
        success_rate,
        config.profile,
        privacy_score,
        dnssec,
        blocks_ads,
        family_safe,
    );

    ServerBenchmarkResult {
        server_id,
        name,
        provider,
        primary_ip,
        doh,
        dot,
        transport: config.transport,
        is_current_system_dns: is_current,
        min_ms: (min_ms * 100.0).round() / 100.0,
        median_ms: (median_ms * 100.0).round() / 100.0,
        avg_ms: (avg_ms * 100.0).round() / 100.0,
        p95_ms: (p95_ms * 100.0).round() / 100.0,
        jitter_ms: (jitter_ms * 100.0).round() / 100.0,
        success_rate: (success_rate * 1000.0).round() / 1000.0,
        queries_sent: sent,
        queries_ok: ok,
        dnssec,
        privacy_score,
        blocks_ads,
        family_safe,
        smart_score: (smart_score * 100.0).round() / 100.0,
    }
}

pub async fn run_benchmark(catalog: &Catalog, config: BenchmarkConfig) -> BenchmarkReport {
    let client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .unwrap_or_default(),
    );

    let candidates: Vec<&DnsServer> = catalog
        .filter_by_profile(config.profile)
        .into_iter()
        .filter(|s| match config.transport {
            Transport::Udp | Transport::Tcp => !s.ipv4.is_empty(),
            Transport::Doh => s.doh.is_some(),
            Transport::Dot => s.dot.is_some() && !s.ipv4.is_empty(),
        })
        .collect();

    let sem = Arc::new(Semaphore::new(config.concurrency.max(1)));
    let mut handles = Vec::new();

    for server in candidates {
        let Some(ip) = server.primary_ipv4().map(|s| s.to_string()) else {
            continue;
        };
        let permit_sem = sem.clone();
        let client_clone = client.clone();
        let cfg = config.clone();

        let id = server.id.clone();
        let name = server.name.clone();
        let provider = server.provider.clone();
        let doh = server.doh.clone();
        let dot = server.dot.clone();
        let dnssec = server.dnssec;
        let privacy_score = server.privacy_score;
        let blocks_ads = server.blocks_ads();
        let family_safe = server.is_family_safe();

        handles.push(tokio::spawn(async move {
            let _permit = permit_sem.acquire().await;
            bench_single_target(
                client_clone,
                id,
                name,
                provider,
                ip,
                doh,
                dot,
                false,
                dnssec,
                privacy_score,
                blocks_ads,
                family_safe,
                &cfg,
            )
            .await
        }));
    }

    // Optionally benchmark current system DNS (only for UDP/TCP)
    if config.include_current_dns && matches!(config.transport, Transport::Udp | Transport::Tcp) {
        if let Ok(status) = get_current_system_dns() {
            if let Some(current_ip) = status.active_servers.first().cloned() {
                let already_tested = catalog.find_by_ip(&current_ip).is_some();
                if !already_tested {
                    let permit_sem = sem.clone();
                    let client_clone = client.clone();
                    let cfg = config.clone();
                    handles.push(tokio::spawn(async move {
                        let _permit = permit_sem.acquire().await;
                        bench_single_target(
                            client_clone,
                            "current-system-dns".into(),
                            format!("Current DNS ({current_ip})"),
                            "ISP / System Default".into(),
                            current_ip,
                            None,
                            None,
                            true,
                            false,
                            2,
                            false,
                            false,
                            &cfg,
                        )
                        .await
                    }));
                }
            }
        }
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(res) = handle.await {
            results.push(res);
        }
    }

    // Sort reachable servers first, by smart_score descending then median_ms ascending
    results.sort_by(|a, b| {
        let a_ok = a.queries_ok > 0;
        let b_ok = b.queries_ok > 0;
        b_ok.cmp(&a_ok)
            .then_with(|| {
                b.smart_score
                    .partial_cmp(&a.smart_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                a.median_ms
                    .partial_cmp(&b.median_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let recommended = results
        .iter()
        .find(|r| r.queries_ok > 0 && !r.is_current_system_dns)
        .cloned();

    BenchmarkReport {
        timestamp: Utc::now(),
        profile: config.profile,
        transport: config.transport,
        results,
        recommended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_score_favors_faster_reliable_server() {
        let fast = compute_smart_score(8.0, 1.0, GoalProfile::Speed, 4, true, false, false);
        let slow = compute_smart_score(65.0, 1.0, GoalProfile::Speed, 4, true, false, false);
        assert!(fast > slow);
    }

    #[test]
    fn smart_score_zero_when_unreachable() {
        let dead = compute_smart_score(9999.0, 0.0, GoalProfile::Speed, 5, true, true, true);
        assert_eq!(dead, 0.0);
    }
}
