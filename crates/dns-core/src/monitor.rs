use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::benchmark::{run_benchmark, BenchmarkConfig, BenchmarkReport};
use crate::catalog::{Catalog, GoalProfile};
use crate::changer::{apply_dns, config_dir, get_current_system_dns};
use crate::protocol::{query_udp, RecordType, ResponseCode, Transport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub profile: GoalProfile,
    pub transport: Transport,
    pub winner_id: String,
    pub winner_name: String,
    pub winner_ip: String,
    pub winner_median_ms: f64,
    pub servers_tested: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorTick {
    pub timestamp: DateTime<Utc>,
    pub active_ip: String,
    pub healthy: bool,
    pub latency_ms: Option<f64>,
    pub switched_to: Option<String>,
}

pub fn history_path() -> PathBuf {
    config_dir().join("history.jsonl")
}

pub fn append_history(report: &BenchmarkReport) -> std::io::Result<Option<HistoryEntry>> {
    let Some(ref winner) = report.recommended else {
        return Ok(None);
    };
    let entry = HistoryEntry {
        timestamp: report.timestamp,
        profile: report.profile,
        transport: report.transport,
        winner_id: winner.server_id.clone(),
        winner_name: winner.name.clone(),
        winner_ip: winner.primary_ip.clone(),
        winner_median_ms: winner.median_ms,
        servers_tested: report.results.len(),
    };
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path())?;
    let line = serde_json::to_string(&entry).unwrap_or_default();
    writeln!(file, "{line}")?;
    Ok(Some(entry))
}

pub fn read_history(limit: usize) -> std::io::Result<Vec<HistoryEntry>> {
    let path = history_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&line) {
            entries.push(entry);
        }
    }
    entries.reverse();
    entries.truncate(limit);
    Ok(entries)
}

/// Probe the currently active DNS server. If it fails and `auto_switch` is true,
/// benchmark healthy resolvers for the chosen profile and switch to the fastest reachable one.
pub async fn check_health_and_failover(
    catalog: &Catalog,
    profile: GoalProfile,
    auto_switch: bool,
    dry_run: bool,
) -> MonitorTick {
    let status = get_current_system_dns().unwrap_or_default();
    let active_ip = status
        .active_servers
        .first()
        .cloned()
        .unwrap_or_else(|| "1.1.1.1".to_string());

    let probe = query_udp(&active_ip, "example.com", RecordType::A, Duration::from_secs(2)).await;
    match probe {
        Ok(resp) if resp.rcode == ResponseCode::NoError => MonitorTick {
            timestamp: Utc::now(),
            active_ip,
            healthy: true,
            latency_ms: Some((resp.latency_ms * 100.0).round() / 100.0),
            switched_to: None,
        },
        _ => {
            let mut switched_to = None;
            if auto_switch {
                let report = run_benchmark(
                    catalog,
                    BenchmarkConfig {
                        profile,
                        runs_per_domain: 1,
                        domains: vec!["example.com".into(), "cloudflare.com".into()],
                        include_current_dns: false,
                        ..Default::default()
                    },
                )
                .await;
                if let Some(best) = report.recommended {
                    if let Some(srv) = catalog.find_by_id(&best.server_id) {
                        if apply_dns(srv, None, dry_run).is_ok() {
                            switched_to = Some(format!("{} ({})", srv.name, best.primary_ip));
                        }
                    }
                }
            }
            MonitorTick {
                timestamp: Utc::now(),
                active_ip,
                healthy: false,
                latency_ms: None,
                switched_to,
            }
        }
    }
}
