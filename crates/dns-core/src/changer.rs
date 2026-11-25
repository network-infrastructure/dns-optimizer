use std::fs;
use std::path::PathBuf;
use std::process::Command;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::catalog::DnsServer;

#[derive(Debug, Error)]
pub enum ChangerError {
    #[error("no active network interface detected")]
    NoInterface,
    #[error("command failed ({0}): {1}")]
    CommandFailed(String, String),
    #[error("no backup found to restore at {0}")]
    NoBackup(String),
    #[error("server has no IPv4 addresses configured")]
    NoServerAddresses,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemDnsStatus {
    pub os: String,
    pub interface: String,
    pub active_servers: Vec<String>,
    pub has_backup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsBackup {
    pub saved_at: DateTime<Utc>,
    pub os: String,
    pub interface: String,
    pub original_servers: Vec<String>,
    pub was_dhcp_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub interface: String,
    pub applied_servers: Vec<String>,
    pub dot_hostname: Option<String>,
    pub doh_url: Option<String>,
    pub dry_run: bool,
    pub commands: Vec<String>,
    pub backup_saved: bool,
}

pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("dns-optimizer")
}

pub fn backup_path() -> PathBuf {
    config_dir().join("dns-backup.json")
}

fn save_backup(backup: &DnsBackup) -> Result<(), ChangerError> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    let path = backup_path();
    // Only write initial backup if one does not already exist so original ISP settings are preserved.
    if !path.exists() {
        let json = serde_json::to_string_pretty(backup)?;
        fs::write(&path, json)?;
    }
    Ok(())
}

pub fn load_backup() -> Result<DnsBackup, ChangerError> {
    let path = backup_path();
    if !path.exists() {
        return Err(ChangerError::NoBackup(path.display().to_string()));
    }
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn run_cmd(program: &str, args: &[&str]) -> Result<String, ChangerError> {
    let output = Command::new(program).args(args).output()?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if !stderr.is_empty() { stderr } else { stdout };
        Err(ChangerError::CommandFailed(
            format!("{program} {}", args.join(" ")),
            msg,
        ))
    }
}

#[cfg(target_os = "macos")]
fn detect_default_interface() -> Result<String, ChangerError> {
    let list = run_cmd("networksetup", &["-listallnetworkservices"])?;
    for line in list.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("Wi-Fi") || trimmed.contains("Ethernet") || trimmed.contains("USB") {
            return Ok(trimmed.to_string());
        }
    }
    list.lines()
        .skip(1)
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with('*'))
        .map(|s| s.to_string())
        .ok_or(ChangerError::NoInterface)
}

#[cfg(target_os = "macos")]
pub fn get_current_system_dns() -> Result<SystemDnsStatus, ChangerError> {
    let iface = detect_default_interface().unwrap_or_else(|_| "Wi-Fi".to_string());
    let mut servers = Vec::new();
    if let Ok(out) = run_cmd("networksetup", &["-getdnsservers", &iface]) {
        for line in out.lines() {
            let t = line.trim();
            if t.parse::<std::net::IpAddr>().is_ok() {
                servers.push(t.to_string());
            }
        }
    }
    if servers.is_empty() {
        if let Ok(out) = run_cmd("scutil", &["--dns"]) {
            for line in out.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("nameserver[") {
                    if let Some((_, ip)) = rest.split_once(':') {
                        let ip_t = ip.trim();
                        if ip_t.parse::<std::net::IpAddr>().is_ok()
                            && !servers.contains(&ip_t.to_string())
                        {
                            servers.push(ip_t.to_string());
                        }
                    }
                }
            }
        }
    }
    Ok(SystemDnsStatus {
        os: "macos".to_string(),
        interface: iface,
        active_servers: servers,
        has_backup: backup_path().exists(),
    })
}

#[cfg(target_os = "windows")]
fn detect_default_interface() -> Result<String, ChangerError> {
    let out = run_cmd(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | Select-Object -First 1 -ExpandProperty Name)",
        ],
    )?;
    if out.is_empty() {
        Err(ChangerError::NoInterface)
    } else {
        Ok(out)
    }
}

#[cfg(target_os = "windows")]
pub fn get_current_system_dns() -> Result<SystemDnsStatus, ChangerError> {
    let iface = detect_default_interface().unwrap_or_else(|_| "Ethernet".to_string());
    let mut servers = Vec::new();
    let script = format!(
        "(Get-DnsClientServerAddress -InterfaceAlias '{}' -AddressFamily IPv4).ServerAddresses -join \"`n\"",
        iface.replace('\'', "''")
    );
    if let Ok(out) = run_cmd("powershell", &["-NoProfile", "-Command", &script]) {
        for line in out.lines() {
            let t = line.trim();
            if t.parse::<std::net::IpAddr>().is_ok() {
                servers.push(t.to_string());
            }
        }
    }
    Ok(SystemDnsStatus {
        os: "windows".to_string(),
        interface: iface,
        active_servers: servers,
        has_backup: backup_path().exists(),
    })
}

#[cfg(target_os = "linux")]
fn detect_default_interface() -> Result<String, ChangerError> {
    if let Ok(out) = run_cmd("nmcli", &["-t", "-f", "NAME,DEVICE", "connection", "show", "--active"]) {
        if let Some(first) = out.lines().next() {
            if let Some((name, _)) = first.split_once(':') {
                if !name.is_empty() {
                    return Ok(name.to_string());
                }
            }
        }
    }
    Ok("default".to_string())
}

#[cfg(target_os = "linux")]
pub fn get_current_system_dns() -> Result<SystemDnsStatus, ChangerError> {
    let iface = detect_default_interface().unwrap_or_else(|_| "default".to_string());
    let mut servers = Vec::new();
    if let Ok(content) = fs::read_to_string("/etc/resolv.conf") {
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("nameserver ") {
                let ip = rest.trim();
                if ip.parse::<std::net::IpAddr>().is_ok() {
                    servers.push(ip.to_string());
                }
            }
        }
    }
    Ok(SystemDnsStatus {
        os: "linux".to_string(),
        interface: iface,
        active_servers: servers,
        has_backup: backup_path().exists(),
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn get_current_system_dns() -> Result<SystemDnsStatus, ChangerError> {
    Ok(SystemDnsStatus {
        os: std::env::consts::OS.to_string(),
        interface: "system".to_string(),
        active_servers: vec![],
        has_backup: backup_path().exists(),
    })
}

/// Apply a DNS server from the catalog to the operating system network interface.
/// Saves a backup of original DNS settings before changing anything.
pub fn apply_dns(
    server: &DnsServer,
    interface_override: Option<&str>,
    dry_run: bool,
) -> Result<ApplyResult, ChangerError> {
    if server.ipv4.is_empty() {
        return Err(ChangerError::NoServerAddresses);
    }

    let status = get_current_system_dns().unwrap_or_default();
    let iface = interface_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| status.interface.clone());

    let mut commands = Vec::new();
    let backup = DnsBackup {
        saved_at: Utc::now(),
        os: std::env::consts::OS.to_string(),
        interface: iface.clone(),
        original_servers: status.active_servers.clone(),
        was_dhcp_default: status.active_servers.is_empty(),
    };

    let mut backup_saved = false;
    if !dry_run {
        save_backup(&backup)?;
        backup_saved = true;
    }

    #[cfg(target_os = "macos")]
    {
        let mut args = vec!["-setdnsservers", iface.as_str()];
        for ip in &server.ipv4 {
            args.push(ip.as_str());
        }
        commands.push(format!("networksetup {}", args.join(" ")));
        if !dry_run {
            run_cmd("networksetup", &args)?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let primary = &server.ipv4[0];
        let cmd1 = format!(
            "netsh interface ip set dns name=\"{}\" static {}",
            iface, primary
        );
        commands.push(cmd1);
        if !dry_run {
            run_cmd(
                "netsh",
                &[
                    "interface",
                    "ip",
                    "set",
                    "dns",
                    &format!("name={iface}"),
                    "static",
                    primary,
                ],
            )?;
        }
        if let Some(secondary) = server.ipv4.get(1) {
            let cmd2 = format!(
                "netsh interface ip add dns name=\"{}\" {} index=2",
                iface, secondary
            );
            commands.push(cmd2);
            if !dry_run {
                run_cmd(
                    "netsh",
                    &[
                        "interface",
                        "ip",
                        "add",
                        "dns",
                        &format!("name={iface}"),
                        secondary,
                        "index=2",
                    ],
                )?;
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let joined = server.ipv4.join(" ");
        commands.push(format!(
            "nmcli connection modify \"{iface}\" ipv4.dns \"{joined}\" ipv4.ignore-auto-dns yes"
        ));
        commands.push(format!("nmcli connection up \"{iface}\""));
        if !dry_run {
            run_cmd(
                "nmcli",
                &[
                    "connection",
                    "modify",
                    &iface,
                    "ipv4.dns",
                    &joined,
                    "ipv4.ignore-auto-dns",
                    "yes",
                ],
            )?;
            run_cmd("nmcli", &["connection", "up", &iface])?;
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        if let Some(ref dot) = server.dot {
            commands.push(format!("Android Private DNS hostname: {dot}"));
        }
    }

    Ok(ApplyResult {
        interface: iface,
        applied_servers: server.ipv4.clone(),
        dot_hostname: server.dot.clone(),
        doh_url: server.doh.clone(),
        dry_run,
        commands,
        backup_saved,
    })
}

/// Restore original DNS settings (or DHCP default) from saved backup.
pub fn restore_dns(interface_override: Option<&str>, dry_run: bool) -> Result<ApplyResult, ChangerError> {
    let backup = load_backup().ok();
    let iface = interface_override
        .map(|s| s.to_string())
        .or_else(|| backup.as_ref().map(|b| b.interface.clone()))
        .or_else(|| get_current_system_dns().ok().map(|s| s.interface))
        .ok_or(ChangerError::NoInterface)?;

    let mut commands = Vec::new();
    let restored_servers = backup
        .as_ref()
        .map(|b| b.original_servers.clone())
        .unwrap_or_default();

    #[cfg(target_os = "macos")]
    {
        commands.push(format!("networksetup -setdnsservers \"{iface}\" Empty"));
        if !dry_run {
            run_cmd("networksetup", &["-setdnsservers", &iface, "Empty"])?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        commands.push(format!("netsh interface ip set dns name=\"{iface}\" dhcp"));
        if !dry_run {
            run_cmd(
                "netsh",
                &["interface", "ip", "set", "dns", &format!("name={iface}"), "dhcp"],
            )?;
        }
    }

    #[cfg(target_os = "linux")]
    {
        commands.push(format!(
            "nmcli connection modify \"{iface}\" ipv4.dns \"\" ipv4.ignore-auto-dns no"
        ));
        commands.push(format!("nmcli connection up \"{iface}\""));
        if !dry_run {
            run_cmd(
                "nmcli",
                &[
                    "connection",
                    "modify",
                    &iface,
                    "ipv4.dns",
                    "",
                    "ipv4.ignore-auto-dns",
                    "no",
                ],
            )?;
            run_cmd("nmcli", &["connection", "up", &iface])?;
        }
    }

    if !dry_run && backup_path().exists() {
        let _ = fs::remove_file(backup_path());
    }

    Ok(ApplyResult {
        interface: iface,
        applied_servers: restored_servers,
        dot_hostname: None,
        doh_url: None,
        dry_run,
        commands,
        backup_saved: false,
    })
}
