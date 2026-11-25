use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use dns_core::android::build_android_guide;
use dns_core::{
    append_history, apply_dns, check_health_and_failover, get_current_system_dns, read_history,
    restore_dns, run_adblock_test, run_benchmark, run_privacy_test, BenchmarkConfig, Catalog,
    GoalProfile, Transport,
};

#[derive(Parser)]
#[command(
    name = "dns-optimizer",
    version,
    about = "Cross-platform Rust DNS benchmark, speed test, diagnostics, and DNS changer"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a live DNS query latency benchmark (measures real DNS lookups, not ICMP ping)
    Bench {
        /// Goal profile: speed, privacy, adblock, family
        #[arg(short = 'p', long, default_value = "speed")]
        profile: String,
        /// Transport protocol: udp, tcp, doh, dot
        #[arg(short = 't', long, default_value = "udp")]
        protocol: String,
        /// Runs per domain
        #[arg(short, long, default_value_t = 2)]
        runs: usize,
        /// Timeout per query in milliseconds
        #[arg(long, default_value_t = 2000)]
        timeout_ms: u64,
        /// Maximum number of servers to display
        #[arg(long, default_value_t = 15)]
        top: usize,
        /// Output JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Apply a DNS server from the catalog to the active system network interface
    Apply {
        /// Server ID (e.g. cloudflare-standard, adguard-default) or primary IPv4
        server: String,
        /// Optional network interface name override
        #[arg(short, long)]
        interface: Option<String>,
        /// Preview the OS commands without modifying system settings
        #[arg(long)]
        dry_run: bool,
    },
    /// Restore original DNS settings (or DHCP default) from saved backup
    Restore {
        /// Optional network interface name override
        #[arg(short, long)]
        interface: Option<String>,
        /// Preview the OS commands without modifying system settings
        #[arg(long)]
        dry_run: bool,
    },
    /// Show current system DNS configuration and backup status
    Status {
        #[arg(long)]
        json: bool,
    },
    /// List DNS servers in the embedded catalog
    List {
        /// Filter by profile: speed, privacy, adblock, family
        #[arg(short, long)]
        profile: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Check active system resolver, ISP leak risk, and DNSSEC validation
    TestPrivacy {
        #[arg(long)]
        json: bool,
    },
    /// Test whether a DNS server blocks known ad and tracker domains
    TestAdblock {
        /// Server IPv4 address or catalog ID (defaults to active system DNS or 94.140.14.14)
        #[arg(short, long)]
        server: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Monitor active DNS health and automatically fail over when unreachable
    Monitor {
        /// Check interval in seconds
        #[arg(short, long, default_value_t = 30)]
        interval: u64,
        /// Goal profile for failover target selection
        #[arg(short, long, default_value = "speed")]
        profile: String,
        /// Enable automatic failover to the fastest healthy server
        #[arg(long)]
        failover: bool,
        /// Run once and exit
        #[arg(long)]
        once: bool,
        /// Preview failover commands without applying
        #[arg(long)]
        dry_run: bool,
    },
    /// Show saved DNS benchmark history
    History {
        #[arg(short, long, default_value_t = 15)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    /// Print Android Private DNS (DoT) hostname and setup instructions for a server
    Android {
        /// Server ID (e.g. cloudflare-standard, adguard-default, quad9-recommended)
        server: String,
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let catalog = Catalog::load_embedded().context("failed to load embedded DNS catalog")?;

    match cli.command {
        Commands::Bench {
            profile,
            protocol,
            runs,
            timeout_ms,
            top,
            json,
        } => {
            let goal = GoalProfile::from_str_loose(&profile)
                .with_context(|| format!("unknown profile: {profile}"))?;
            let transport = Transport::from_str_loose(&protocol)
                .with_context(|| format!("unknown protocol: {protocol}"))?;

            if !json {
                println!(
                    "DNS Optimizer — Benchmarking {} profile over {:?} ({} runs/domain)...",
                    goal.label(),
                    transport,
                    runs
                );
            }

            let report = run_benchmark(
                &catalog,
                BenchmarkConfig {
                    profile: goal,
                    transport,
                    runs_per_domain: runs,
                    timeout_ms,
                    ..Default::default()
                },
            )
            .await;

            let _ = append_history(&report);

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
                return Ok(());
            }

            println!(
                "\n{:<28} {:<16} {:>9} {:>9} {:>7} {:>7}",
                "Server", "Primary IP", "Median", "Min", "OK %", "Score"
            );
            println!("{}", "-".repeat(82));
            for row in report.results.iter().take(top) {
                let med = if row.queries_ok > 0 {
                    format!("{:.1} ms", row.median_ms)
                } else {
                    "timeout".to_string()
                };
                let min = if row.queries_ok > 0 {
                    format!("{:.1} ms", row.min_ms)
                } else {
                    "-".to_string()
                };
                println!(
                    "{:<28} {:<16} {:>9} {:>9} {:>6.0}% {:>7.1}",
                    row.name.chars().take(27).collect::<String>(),
                    row.primary_ip,
                    med,
                    min,
                    row.success_rate * 100.0,
                    row.smart_score
                );
            }

            if let Some(rec) = report.recommended {
                println!(
                    "\nRecommended DNS: {} ({}) — {:.1} ms median",
                    rec.name, rec.primary_ip, rec.median_ms
                );
                println!("Apply with: dns-optimizer apply {}", rec.server_id);
            }
        }

        Commands::Apply {
            server,
            interface,
            dry_run,
        } => {
            let Some(srv) = catalog.find_by_id(&server) else {
                bail!("server '{server}' not found in catalog (run `dns-optimizer list`)");
            };
            let res = apply_dns(srv, interface.as_deref(), dry_run)?;
            if dry_run {
                println!("Dry run — no system settings changed.");
            } else {
                println!("Applied {} to interface '{}'.", srv.name, res.interface);
            }
            for cmd in &res.commands {
                println!("  $ {cmd}");
            }
            if let Some(dot) = &res.dot_hostname {
                println!("  DoT hostname: {dot}");
            }
            if let Some(doh) = &res.doh_url {
                println!("  DoH endpoint: {doh}");
            }
        }

        Commands::Restore { interface, dry_run } => {
            let res = restore_dns(interface.as_deref(), dry_run)?;
            if dry_run {
                println!("Dry run — restore commands:");
            } else {
                println!("Restored DNS settings on interface '{}'.", res.interface);
            }
            for cmd in &res.commands {
                println!("  $ {cmd}");
            }
        }

        Commands::Status { json } => {
            let status = get_current_system_dns()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("OS: {}", status.os);
                println!("Interface: {}", status.interface);
                println!(
                    "Active DNS: {}",
                    if status.active_servers.is_empty() {
                        "DHCP / system default".to_string()
                    } else {
                        status.active_servers.join(", ")
                    }
                );
                println!(
                    "Backup saved: {}",
                    if status.has_backup { "yes" } else { "no" }
                );
            }
        }

        Commands::List { profile, json } => {
            let list: Vec<_> = if let Some(p) = profile {
                let goal = GoalProfile::from_str_loose(&p)
                    .with_context(|| format!("unknown profile: {p}"))?;
                catalog.filter_by_profile(goal)
            } else {
                catalog.servers.iter().collect()
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&list)?);
                return Ok(());
            }

            println!(
                "{:<26} {:<16} {:<10} {:<14} {}",
                "ID", "IPv4", "DNSSEC", "Privacy", "Filtering"
            );
            println!("{}", "-".repeat(86));
            for srv in list {
                println!(
                    "{:<26} {:<16} {:<10} {}/5            {}",
                    srv.id.chars().take(25).collect::<String>(),
                    srv.primary_ipv4().unwrap_or("-"),
                    if srv.dnssec { "yes" } else { "no" },
                    srv.privacy_score,
                    srv.filtering.join(", ")
                );
            }
        }

        Commands::TestPrivacy { json } => {
            let report = run_privacy_test(&catalog).await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Active resolvers: {}", report.active_servers.join(", "));
                println!(
                    "Matched providers: {}",
                    if report.matched_known_providers.is_empty() {
                        "None (ISP or local router)".to_string()
                    } else {
                        report.matched_known_providers.join(", ")
                    }
                );
                println!(
                    "DNSSEC validation: {} ({})",
                    if report.dnssec_validated { "PASS" } else { "FAIL" },
                    report.dnssec_probe_rcode
                );
                println!("Leak risk: {}", report.leak_risk);
                println!("Recommendation: {}", report.recommendation);
            }
        }

        Commands::TestAdblock { server, json } => {
            let target_ip = match server {
                Some(ref s) => catalog
                    .find_by_id(s)
                    .and_then(|srv| srv.primary_ipv4().map(|ip| ip.to_string()))
                    .unwrap_or_else(|| s.clone()),
                None => get_current_system_dns()
                    .ok()
                    .and_then(|st| st.active_servers.first().cloned())
                    .unwrap_or_else(|| "94.140.14.14".to_string()),
            };
            let report = run_adblock_test(&target_ip).await;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Ad-blocking test against {} — blocked {}/{} ({:.1}%)",
                    report.server_ip,
                    report.blocked_count,
                    report.total_tested,
                    report.block_rate_percent
                );
                for c in &report.checks {
                    println!(
                        "  [{:<7}] {:<34} rcode={} answers={:?}",
                        if c.blocked { "BLOCKED" } else { "ALLOWED" },
                        c.domain,
                        c.rcode,
                        c.answers
                    );
                }
            }
        }

        Commands::Monitor {
            interval,
            profile,
            failover,
            once,
            dry_run,
        } => {
            let goal = GoalProfile::from_str_loose(&profile)
                .with_context(|| format!("unknown profile: {profile}"))?;
            loop {
                let tick = check_health_and_failover(&catalog, goal, failover, dry_run).await;
                println!(
                    "[{}] active={} healthy={} latency={:?}ms switched={:?}",
                    tick.timestamp.format("%H:%M:%S"),
                    tick.active_ip,
                    tick.healthy,
                    tick.latency_ms,
                    tick.switched_to
                );
                if once {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(interval)).await;
            }
        }

        Commands::History { limit, json } => {
            let entries = read_history(limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else if entries.is_empty() {
                println!("No benchmark history recorded yet. Run `dns-optimizer bench` first.");
            } else {
                println!(
                    "{:<22} {:<10} {:<26} {:>10}",
                    "Timestamp (UTC)", "Profile", "Fastest Server", "Median"
                );
                println!("{}", "-".repeat(72));
                for e in entries {
                    println!(
                        "{:<22} {:<10} {:<26} {:>7.1} ms",
                        e.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        format!("{:?}", e.profile),
                        e.winner_name.chars().take(25).collect::<String>(),
                        e.winner_median_ms
                    );
                }
            }
        }

        Commands::Android { server, json } => {
            let Some(guide) = build_android_guide(&catalog, &server) else {
                bail!("server '{server}' not found in catalog");
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&guide)?);
            } else {
                println!("Android Private DNS setup for {}", guide.server_name);
                if let Some(dot) = &guide.private_dns_hostname {
                    println!("Private DNS hostname: {dot}");
                }
                for (i, step) in guide.setup_steps.iter().enumerate() {
                    println!("  {}. {step}", i + 1);
                }
            }
        }
    }

    Ok(())
}
