# DNS Optimizer

**Best DNS Servers: Free DNS Benchmark, Speed Test & DNS Changer**

Find the fastest and most secure DNS servers for your network. Free, open-source Rust DNS benchmark and DNS changer for Windows, macOS, Linux, and Android.

- Website & live DoH speed test: https://network-infrastructure.github.io/dns-optimizer/
- Source code: https://github.com/network-infrastructure/dns-optimizer
- License: MIT

## Why DNS Optimizer exists

Public DNS rankings based on global averages rarely match what happens on your own ISP. Anycast routing varies by city and peering provider, so a resolver that answers in 6 ms on one connection may take 30 ms on another.

DNS Optimizer measures actual RFC 1035 DNS lookup latency (A and AAAA record resolution) across 40 verified public resolvers and your current ISP default. It does not rely on ICMP ping.

## Core features

| Feature | What it does |
| --- | --- |
| DNS Speed Test | Runs concurrent RFC 1035 queries over UDP/53, TCP/53, DoH, and DoT |
| DNS Changer | Applies primary and secondary DNS to your active OS network adapter |
| Smart DNS | Scores resolvers for Maximum speed, Privacy, Block ads, or Family protection |
| DNS Privacy Test | Inspects active system resolvers and checks DNSSEC validation (`dnssec-failed.org`) |
| Ad Blocking Test | Queries known ad and tracker domains to verify `0.0.0.0` or `NXDOMAIN` sinkholing |
| Family DNS | Benchmarks and configures adult-content and malware filtering resolvers |
| DNS Encryption | Supports DNS-over-HTTPS (RFC 8484) and DNS-over-TLS (RFC 7858 port 853) |
| Auto Switch | Monitors active DNS health and fails over to the fastest healthy server |
| DNS History | Records timestamped benchmark winners in local JSONL storage |
| Restore DNS | Saves your original network configuration before switching and restores it anytime |

## Cross-platform Rust architecture

One Rust core crate (`crates/dns-core`) powers every platform target:

| Platform | Target / Stack | Capabilities |
| --- | --- | --- |
| Windows | Rust + Tauri v2 / CLI (`netsh`, PowerShell) | Full UDP/TCP/DoH/DoT benchmark, adapter switch, backup & restore |
| macOS | Rust + Tauri v2 / CLI (`networksetup`, `scutil`) | Full benchmark, interface switch, DHCP restore |
| Linux | Rust CLI (`nmcli`, `/etc/resolv.conf`) | Benchmark, switching, failover daemon, JSON output |
| Android | Rust C-ABI FFI (`aarch64-linux-android`) + Tauri v2 Mobile | Private DNS (DoT) hostnames, ad-blocking and family profiles |
| Web | Static GitHub Pages (`docs/`) | Live browser DoH speed test, DNS record checker, 40-server catalog |

Web browsers cannot open raw UDP port 53 sockets or change operating system network adapters. The web version tests CORS-enabled DoH endpoints, while the downloadable Rust app handles full UDP/53, TCP/53, DoT, DoH diagnostics and system DNS switching.

## Quick start (Rust CLI)

Requirements: Rust 1.80+ (`cargo`).

```bash
git clone https://github.com/network-infrastructure/dns-optimizer.git
cd dns-optimizer

# Benchmark resolvers for maximum speed (UDP/53)
cargo run --release --bin dns-optimizer -- bench --profile speed

# Benchmark privacy resolvers over DNS-over-TLS (port 853)
cargo run --release --bin dns-optimizer -- bench --profile privacy --protocol dot

# Preview the OS command without changing system settings
cargo run --release --bin dns-optimizer -- apply cloudflare-standard --dry-run

# Apply a resolver (requires Administrator on Windows or sudo on macOS/Linux)
cargo run --release --bin dns-optimizer -- apply cloudflare-standard

# Restore original DNS / DHCP settings
cargo run --release --bin dns-optimizer -- restore

# Run privacy & DNSSEC validation test
cargo run --release --bin dns-optimizer -- test-privacy

# Test whether a resolver blocks advertising domains
cargo run --release --bin dns-optimizer -- test-adblock --server 94.140.14.14

# Print Android Private DNS (DoT) setup instructions
cargo run --release --bin dns-optimizer -- android adguard-default

# Monitor health and fail over automatically
cargo run --release --bin dns-optimizer -- monitor --interval 30 --failover
```

## Running the desktop & Android app (Tauri v2)

```bash
cd apps/desktop
npm install
npm run dev
```

For Android builds with Tauri v2:

```bash
cd apps/desktop
npm run android:init
npm run android:build
```

## Workspace layout

```text
dns-optimizer/
├── crates/
│   ├── dns-core/          # Shared Rust DNS engine (protocol, benchmark, diagnostics, changer, monitor, Android FFI)
│   └── dns-cli/           # Cross-platform CLI binary (`dns-optimizer`)
├── apps/
│   └── desktop/           # Tauri v2 GUI for Windows, macOS, Linux, and Android
├── data/
│   └── dns_servers.json   # 40 verified public DNS servers
├── docs/                  # GitHub Pages site (20 SEO routes + live browser DoH tester)
└── .github/workflows/     # Cross-platform CI and release builds
```

## Honest note about gaming ping

Switching DNS speeds up domain resolution, matchmaking host lookups, store pages, and CDN selection. It does not lower in-game ping after your computer or console has already connected to a match server.

## Testing

```bash
cargo test --workspace
```
