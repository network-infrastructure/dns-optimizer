use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::catalog::Catalog;
use crate::changer::get_current_system_dns;
use crate::protocol::{query_udp, RecordType, ResponseCode};

pub const ADBLOCK_TEST_DOMAINS: &[&str] = &[
    "doubleclick.net",
    "pagead2.googlesyndication.com",
    "adservice.google.com",
    "ads.pubmatic.com",
    "analytics.google.com",
    "pixel.advertising.com",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyTestReport {
    pub active_servers: Vec<String>,
    pub matched_known_providers: Vec<String>,
    pub uses_known_public_resolver: bool,
    pub dnssec_validated: bool,
    pub dnssec_probe_rcode: String,
    pub leak_risk: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainBlockCheck {
    pub domain: String,
    pub blocked: bool,
    pub rcode: String,
    pub answers: Vec<String>,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBlockTestReport {
    pub server_ip: String,
    pub total_tested: usize,
    pub blocked_count: usize,
    pub block_rate_percent: f64,
    pub checks: Vec<DomainBlockCheck>,
}

fn is_sinkhole_response(rcode: ResponseCode, answers: &[String]) -> bool {
    if matches!(rcode, ResponseCode::NxDomain | ResponseCode::Refused) {
        return true;
    }
    if answers.is_empty() {
        return false;
    }
    answers
        .iter()
        .all(|a| a == "0.0.0.0" || a == "::" || a == "127.0.0.1")
}

/// Inspect active system DNS, check if it belongs to a known privacy resolver,
/// and validate DNSSEC enforcement via dnssec-failed.org.
pub async fn run_privacy_test(catalog: &Catalog) -> PrivacyTestReport {
    let status = get_current_system_dns().unwrap_or_default();
    let mut matched = Vec::new();

    for ip in &status.active_servers {
        if let Some(srv) = catalog.find_by_ip(ip) {
            matched.push(format!("{} ({})", srv.name, srv.provider));
        }
    }

    let probe_ip = status
        .active_servers
        .first()
        .cloned()
        .unwrap_or_else(|| "1.1.1.1".to_string());

    // dnssec-failed.org intentionally has a broken DNSSEC chain.
    // A validating resolver MUST return SERVFAIL.
    let (dnssec_validated, dnssec_probe_rcode) =
        match query_udp(&probe_ip, "dnssec-failed.org", RecordType::A, Duration::from_secs(3)).await
        {
            Ok(resp) => (
                resp.rcode == ResponseCode::ServFail,
                format!("{:?}", resp.rcode),
            ),
            Err(e) => (false, format!("Error: {e}")),
        };

    let uses_known = !matched.is_empty();
    let leak_risk = if uses_known && dnssec_validated {
        "Low".to_string()
    } else if uses_known {
        "Moderate".to_string()
    } else {
        "Elevated (ISP or local router resolver active)".to_string()
    };

    let recommendation = if uses_known && dnssec_validated {
        "System DNS uses a recognized public resolver with active DNSSEC validation.".to_string()
    } else if !uses_known {
        "System DNS points to an ISP or local router IP. Switch to a no-log resolver with DoH or DoT enabled.".to_string()
    } else {
        "Resolver did not reject dnssec-failed.org with SERVFAIL. Consider switching to Cloudflare, Quad9, or Mullvad.".to_string()
    };

    PrivacyTestReport {
        active_servers: status.active_servers,
        matched_known_providers: matched,
        uses_known_public_resolver: uses_known,
        dnssec_validated,
        dnssec_probe_rcode,
        leak_risk,
        recommendation,
    }
}

/// Test whether a given DNS server IP filters known advertising and tracking domains.
pub async fn run_adblock_test(server_ip: &str) -> AdBlockTestReport {
    let mut checks = Vec::new();
    let mut blocked_count = 0usize;

    for domain in ADBLOCK_TEST_DOMAINS {
        match query_udp(server_ip, domain, RecordType::A, Duration::from_secs(2)).await {
            Ok(resp) => {
                let blocked = is_sinkhole_response(resp.rcode, &resp.answers);
                if blocked {
                    blocked_count += 1;
                }
                checks.push(DomainBlockCheck {
                    domain: (*domain).to_string(),
                    blocked,
                    rcode: format!("{:?}", resp.rcode),
                    answers: resp.answers,
                    latency_ms: (resp.latency_ms * 100.0).round() / 100.0,
                });
            }
            Err(_) => {
                checks.push(DomainBlockCheck {
                    domain: (*domain).to_string(),
                    blocked: false,
                    rcode: "Timeout".to_string(),
                    answers: vec![],
                    latency_ms: 0.0,
                });
            }
        }
    }

    let total = checks.len();
    let rate = if total > 0 {
        ((blocked_count as f64 / total as f64) * 1000.0).round() / 10.0
    } else {
        0.0
    };

    AdBlockTestReport {
        server_ip: server_ip.to_string(),
        total_tested: total,
        blocked_count,
        block_rate_percent: rate,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_sinkhole_answers() {
        assert!(is_sinkhole_response(ResponseCode::NxDomain, &[]));
        assert!(is_sinkhole_response(ResponseCode::Refused, &[]));
        assert!(is_sinkhole_response(
            ResponseCode::NoError,
            &["0.0.0.0".to_string()]
        ));
        assert!(!is_sinkhole_response(
            ResponseCode::NoError,
            &["142.250.185.78".to_string()]
        ));
    }
}
