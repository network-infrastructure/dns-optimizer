use dns_core::android::{build_android_guide, AndroidPrivateDnsGuide};
use dns_core::{
    append_history, apply_dns, check_health_and_failover, get_current_system_dns, read_history,
    restore_dns, run_adblock_test, run_benchmark, run_privacy_test, AdBlockTestReport, ApplyResult,
    BenchmarkConfig, BenchmarkReport, Catalog, DnsServer, GoalProfile, HistoryEntry, MonitorTick,
    PrivacyTestReport, SystemDnsStatus, Transport,
};

#[tauri::command]
fn list_servers(profile: Option<String>) -> Result<Vec<DnsServer>, String> {
    let catalog = Catalog::load_embedded().map_err(|e| e.to_string())?;
    if let Some(p) = profile {
        let goal = GoalProfile::from_str_loose(&p).ok_or_else(|| format!("unknown profile: {p}"))?;
        Ok(catalog
            .filter_by_profile(goal)
            .into_iter()
            .cloned()
            .collect())
    } else {
        Ok(catalog.servers)
    }
}

#[tauri::command]
async fn start_benchmark(profile: String, protocol: String) -> Result<BenchmarkReport, String> {
    let catalog = Catalog::load_embedded().map_err(|e| e.to_string())?;
    let goal = GoalProfile::from_str_loose(&profile).unwrap_or(GoalProfile::Speed);
    let transport = Transport::from_str_loose(&protocol).unwrap_or(Transport::Udp);

    let report = run_benchmark(
        &catalog,
        BenchmarkConfig {
            profile: goal,
            transport,
            runs_per_domain: 2,
            ..Default::default()
        },
    )
    .await;

    let _ = append_history(&report);
    Ok(report)
}

#[tauri::command]
fn apply_dns_server(server_id: String, dry_run: bool) -> Result<ApplyResult, String> {
    let catalog = Catalog::load_embedded().map_err(|e| e.to_string())?;
    let srv = catalog
        .find_by_id(&server_id)
        .ok_or_else(|| format!("server '{server_id}' not found"))?;
    apply_dns(srv, None, dry_run).map_err(|e| e.to_string())
}

#[tauri::command]
fn restore_system_dns(dry_run: bool) -> Result<ApplyResult, String> {
    restore_dns(None, dry_run).map_err(|e| e.to_string())
}

#[tauri::command]
fn system_status() -> Result<SystemDnsStatus, String> {
    get_current_system_dns().map_err(|e| e.to_string())
}

#[tauri::command]
async fn privacy_test() -> Result<PrivacyTestReport, String> {
    let catalog = Catalog::load_embedded().map_err(|e| e.to_string())?;
    Ok(run_privacy_test(&catalog).await)
}

#[tauri::command]
async fn adblock_test(server_ip: String) -> Result<AdBlockTestReport, String> {
    Ok(run_adblock_test(&server_ip).await)
}

#[tauri::command]
fn benchmark_history(limit: usize) -> Result<Vec<HistoryEntry>, String> {
    read_history(limit).map_err(|e| e.to_string())
}

#[tauri::command]
async fn health_check(profile: String, auto_switch: bool) -> Result<MonitorTick, String> {
    let catalog = Catalog::load_embedded().map_err(|e| e.to_string())?;
    let goal = GoalProfile::from_str_loose(&profile).unwrap_or(GoalProfile::Speed);
    Ok(check_health_and_failover(&catalog, goal, auto_switch, false).await)
}

#[tauri::command]
fn android_setup(server_id: String) -> Result<AndroidPrivateDnsGuide, String> {
    let catalog = Catalog::load_embedded().map_err(|e| e.to_string())?;
    build_android_guide(&catalog, &server_id)
        .ok_or_else(|| format!("server '{server_id}' not found"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_servers,
            start_benchmark,
            apply_dns_server,
            restore_system_dns,
            system_status,
            privacy_test,
            adblock_test,
            benchmark_history,
            health_check,
            android_setup
        ])
        .run(tauri::generate_context!())
        .expect("error while running DNS Optimizer");
}
