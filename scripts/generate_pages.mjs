import fs from "node:fs";
import path from "node:path";

const ROOT = "/Users/alekseykhokhlov/repo/github/dns-optimizer";
const DOCS = path.join(ROOT, "docs");
const SITE_URL = "https://network-infrastructure.github.io/dns-optimizer";
const REPO_URL = "https://github.com/network-infrastructure/dns-optimizer";

const catalog = JSON.parse(fs.readFileSync(path.join(DOCS, "dns_servers.json"), "utf8"));
const servers = catalog.servers || [];

function byIds(ids) {
  return ids.map((id) => servers.find((s) => s.id === id)).filter(Boolean);
}

function renderServerTable(list) {
  const rows = list
    .map((s) => {
      const ip4 = (s.ipv4 || []).join(", ") || "-";
      const dot = s.dot || "-";
      const filt = (s.filtering || []).join(", ") || "None";
      return `<tr>
        <td>${s.name}</td>
        <td>${ip4}</td>
        <td>${dot}</td>
        <td>${s.dnssec ? "Yes" : "No"}</td>
        <td>${filt}</td>
      </tr>`;
    })
    .join("\n");

  return `<div class="table-wrap">
    <table>
      <thead>
        <tr>
          <th>Resolver</th>
          <th>IPv4</th>
          <th>DoT hostname</th>
          <th>DNSSEC</th>
          <th>Filtering</th>
        </tr>
      </thead>
      <tbody>
        ${rows}
      </tbody>
    </table>
  </div>`;
}

function renderBenchWidget(profile, label) {
  return `<div class="board-shell" data-bench-board data-initial-profile="${profile}">
    <div class="board-top">
      <div class="board-title" data-bench-sub>${label} (demo values before live DoH test)</div>
      <button type="button" class="btn btn--primary btn--sm" data-bench-start>Start DNS Speed Test</button>
    </div>
    <div class="choice-row" role="group" aria-label="DNS goal profile">
      <button type="button" class="choice-btn" data-profile="speed">Speed</button>
      <button type="button" class="choice-btn" data-profile="privacy">Privacy</button>
      <button type="button" class="choice-btn" data-profile="adblock">Ad block</button>
      <button type="button" class="choice-btn" data-profile="family">Family</button>
    </div>
    <div class="board-rows" data-bench-rows></div>
    <div class="board-plate">
      <div>
        <div class="plate-label">Recommended DNS</div>
        <div class="plate-winner" data-bench-winner>Cloudflare (1.1.1.1)</div>
        <div class="plate-label" data-bench-note>Browser tests DoH. Run the Rust app for UDP/53 and one-click apply.</div>
      </div>
      <div class="plate-figure" data-bench-figure>8 ms</div>
    </div>
  </div>`;
}

const PAGES = [
  {
    slug: "dns-speed-test",
    title: "DNS Speed Test: Measure Live Resolver Latency From Your Network",
    description: "Test real DNS query response times from your network. Run a browser DoH speed test or benchmark UDP/53, TCP/53, DoT, and DoH with the open-source Rust app.",
    h1: "DNS speed test",
    lead: "Measure actual DNS lookup time across public resolvers from your current connection.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --protocol udp --runs 3",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "adguard-default", "controld-unfiltered", "nextdns"],
    body: `
      <h2>Why real DNS lookups differ from ping</h2>
      <p>An ICMP ping only measures network round-trip time to an IP address. A real DNS speed test sends RFC 1035 domain queries and measures how fast the resolver returns an A or AAAA record from cache or upstream.</p>
      <h2>Browser DoH test vs Rust desktop test</h2>
      <p>Web browsers cannot open raw UDP port 53 sockets or modify system network adapters. The browser test above times CORS-enabled DNS-over-HTTPS endpoints. Install the Rust CLI or desktop app to benchmark UDP/53, TCP/53, DoT, and your current ISP resolver side by side.</p>
    `
  },
  {
    slug: "dns-benchmark",
    title: "DNS Benchmark: Open-Source Rust Tool for Windows, macOS, and Linux",
    description: "Run a multi-domain DNS benchmark in Rust. Compare median, minimum, p95 latency, jitter, and success rate across 40 public DNS servers.",
    h1: "DNS benchmark",
    lead: "Compare median latency, p95, jitter, and reliability across 40 verified resolvers.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --runs 5 --top 15",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "mullvad-standard", "adguard-default", "opendns-home"],
    body: `
      <h2>What the Rust benchmark engine measures</h2>
      <p>DNS Optimizer dispatches concurrent queries across multiple domains using Tokio async sockets. For every resolver, it reports minimum, median, average, p95 latency, jitter, and packet success rate alongside your active ISP DNS.</p>
      <h2>JSON output for scripts and CI</h2>
      <p>Pass <code>--json</code> to write structured benchmark reports for monitoring pipelines, ISP comparisons, or automated failover checks.</p>
    `
  },
  {
    slug: "dns-changer",
    title: "DNS Changer: Switch System DNS Safely on Windows, macOS, and Linux",
    description: "Free open-source DNS changer written in Rust. Apply the fastest DNS server in one command or click, with automatic backup and one-click restore.",
    h1: "DNS changer",
    lead: "Apply a verified resolver to your network adapter and keep a local backup to restore anytime.",
    profile: "speed",
    showBench: false,
    cli: "dns-optimizer apply cloudflare-standard --dry-run\ndns-optimizer apply cloudflare-standard\ndns-optimizer restore",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "adguard-default", "mullvad-standard"],
    body: `
      <h2>Safe switching with automatic backup</h2>
      <p>Before changing any adapter, DNS Optimizer saves your active nameservers to a local backup file. Running <code>dns-optimizer restore</code> returns your interface to its previous static servers or DHCP default.</p>
      <h2>Native OS integration</h2>
      <p>On Windows the tool uses <code>netsh</code> and PowerShell adapter detection. On macOS it updates the active service with <code>networksetup</code>. On Linux it configures NetworkManager via <code>nmcli</code>.</p>
    `
  },
  {
    slug: "fastest-dns",
    title: "Fastest DNS Server: How to Find the Lowest Latency Resolver for You",
    description: "There is no single fastest DNS server for every city. Test Anycast resolvers like Cloudflare, Google, Quad9, and Control D from your own network.",
    h1: "Fastest DNS server",
    lead: "Anycast routing depends on your ISP peering, so the fastest resolver is always local to your network.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --top 10",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "controld-unfiltered", "opendns-home", "adguard-default"],
    body: `
      <h2>Why global averages mislead</h2>
      <p>Public DNS providers route traffic using BGP Anycast. Cloudflare might answer in 6 ms from one fiber provider and 28 ms from another ISP across town. Testing directly from your machine gives the only number that matters.</p>
      <h2>Warm cache vs cold lookups</h2>
      <p>Popular domains usually hit the resolver cache in a few milliseconds. Uncached domains depend on how quickly the resolver reaches authoritative nameservers.</p>
    `
  },
  {
    slug: "gaming",
    title: "Best DNS for Gaming: Faster Matchmaking Lookups Without False Promises",
    description: "Find reliable, low-latency DNS servers for PC and console gaming. Understand what DNS actually improves and why it does not lower active in-game ping.",
    h1: "Best DNS for gaming",
    lead: "Pick a stable, low-latency resolver for faster game store downloads and matchmaking lookups.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --runs 4",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "controld-unfiltered", "opendns-home"],
    body: `
      <h2>Honest note about in-game ping</h2>
      <p>Changing DNS does not lower your in-game ping once your PC or console has connected to a match server. Active game traffic travels directly to the server IP address without consulting DNS again.</p>
      <h2>What a better DNS actually improves</h2>
      <p>A fast, reliable resolver speeds up lobby host lookups, store pages, patch CDN selection, and login servers. It also avoids ISP DNS timeouts that cause failed matchmaking requests.</p>
    `
  },
  {
    slug: "ps5",
    title: "Best DNS for PS5: Recommended IPv4 Addresses and Setup Steps",
    description: "Configure fast, reliable DNS servers on PlayStation 5. Step-by-step manual DNS settings for Cloudflare, Google DNS, and Quad9.",
    h1: "Best DNS for PS5",
    lead: "Test resolvers on your home network first, then enter the fastest primary and secondary IPv4 on PlayStation 5.",
    profile: "speed",
    showBench: false,
    cli: "dns-optimizer bench --profile speed --top 5",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "opendns-home"],
    body: `
      <h2>How to change DNS on PlayStation 5</h2>
      <p>Open Settings, choose Network, select Settings, and pick Set Up Internet Connection. Press the Options button on your active Wi-Fi or LAN connection, choose Advanced Settings, set DNS Settings to Manual, and enter your Primary and Secondary DNS addresses.</p>
      <h2>What to expect on PS5</h2>
      <p>Manual DNS can make PlayStation Store pages and CDN patch downloads connect more reliably when your ISP resolver is slow. It will not reduce active match ping after a game session starts.</p>
    `
  },
  {
    slug: "xbox",
    title: "Best DNS for Xbox Series X|S: Setup Guide and Server List",
    description: "Set custom DNS servers on Xbox Series X, Series S, or Xbox One. Benchmark resolvers from your router network and apply the winner manually.",
    h1: "Best DNS for Xbox",
    lead: "Benchmark resolvers from a computer on the same router, then apply the fastest IPv4 pair on Xbox.",
    profile: "speed",
    showBench: false,
    cli: "dns-optimizer bench --profile speed --top 5",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "opendns-home"],
    body: `
      <h2>How to change DNS on Xbox</h2>
      <p>Press the Xbox button, go to Profile &amp; system, open Settings, select General, and choose Network settings. Open Advanced settings, choose DNS settings, pick Manual, and type the Primary and Secondary IPv4 addresses.</p>
      <h2>Realistic expectations</h2>
      <p>Better DNS helps Xbox Live sign-in, Game Pass catalog browsing, and CDN routing. Once a multiplayer match connects, latency depends on your ISP route to the game server rather than DNS.</p>
    `
  },
  {
    slug: "android",
    title: "Best DNS for Android: Private DNS (DoT) Hostnames and Ad Blocking",
    description: "Configure encrypted Private DNS on Android 9 and newer with zero extra apps, or build Android support from the DNS Optimizer Rust core.",
    h1: "Best DNS for Android",
    lead: "Encrypt every DNS query on Wi-Fi and cellular using Android built-in Private DNS over TLS.",
    profile: "adblock",
    showBench: true,
    cli: "dns-optimizer android adguard-default",
    serverIds: ["adguard-default", "cloudflare-standard", "quad9-recommended", "mullvad-adblock", "nextdns", "controld-ads"],
    body: `
      <h2>How to enable Private DNS on Android</h2>
      <p>Open Settings, tap Network &amp; internet (or Connections on Samsung devices), select Private DNS, choose Private DNS provider hostname, and paste a DoT hostname such as <code>dns.adguard-dns.com</code> or <code>one.one.one.one</code>.</p>
      <h2>Rust mobile architecture</h2>
      <p>The <code>dns-core</code> Rust crate compiles to <code>aarch64-linux-android</code> with C-ABI exports and Tauri v2 mobile bindings so Android builds share the exact same benchmark and catalog code as desktop.</p>
    `
  },
  {
    slug: "iphone",
    title: "Best DNS for iPhone and iPad: Wi-Fi DNS and Encrypted DoH Profiles",
    description: "Change DNS on iOS for Wi-Fi networks or install encrypted DNS-over-HTTPS and DNS-over-TLS profiles on iPhone and iPad.",
    h1: "Best DNS for iPhone",
    lead: "Set manual DNS per Wi-Fi network or use encrypted DNS-over-HTTPS across Wi-Fi and 5G.",
    profile: "privacy",
    showBench: false,
    cli: "dns-optimizer list --profile privacy",
    serverIds: ["cloudflare-standard", "quad9-recommended", "adguard-default", "mullvad-standard", "nextdns"],
    body: `
      <h2>Manual Wi-Fi DNS on iOS</h2>
      <p>Open Settings, tap Wi-Fi, tap the info icon next to your network, scroll to Configure DNS, switch from Automatic to Manual, remove the router IP, and add your chosen primary and secondary IPv4 addresses.</p>
      <h2>Encrypted DNS across cellular and Wi-Fi</h2>
      <p>Apple supports native DoH and DoT via configuration profiles (.mobileconfig) provided by Cloudflare, Quad9, Mullvad, AdGuard, and NextDNS.</p>
    `
  },
  {
    slug: "windows",
    title: "DNS Changer Windows: Free Open-Source Rust Utility for Windows 10 & 11",
    description: "Benchmark and change DNS on Windows 10 and Windows 11 with a single Rust binary or Tauri desktop app. Automatic backup and one-click DHCP restore.",
    h1: "DNS changer for Windows",
    lead: "Test UDP/53, DoH, and DoT on Windows 10 and 11, then switch adapters with automatic rollback.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer.exe bench --profile speed\ndns-optimizer.exe apply cloudflare-standard\ndns-optimizer.exe restore",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "adguard-default", "controld-ads"],
    body: `
      <h2>How Windows switching works</h2>
      <p>DNS Optimizer detects your active network adapter through PowerShell, saves a backup of your current settings, and runs <code>netsh interface ip set dns</code> with your chosen primary and secondary addresses. Run your terminal or desktop app as Administrator when applying changes.</p>
      <h2>Windows 11 encrypted DNS (DoH)</h2>
      <p>Windows 11 supports native DNS-over-HTTPS for well-known templates including Cloudflare (1.1.1.1), Google (8.8.8.8), and Quad9 (9.9.9.9) under Settings, Network &amp; internet, DNS server assignment.</p>
    `
  },
  {
    slug: "privacy",
    title: "Best DNS for Privacy: No-Log Resolvers With DNSSEC and Encryption",
    description: "Compare privacy-focused public DNS servers with audited no-log policies, DNSSEC validation, QNAME minimization, and no EDNS Client Subnet leaks.",
    h1: "Best DNS for privacy",
    lead: "Stop your ISP from logging every domain you visit by using audited no-log encrypted resolvers.",
    profile: "privacy",
    showBench: true,
    cli: "dns-optimizer bench --profile privacy --protocol dot",
    serverIds: ["mullvad-standard", "quad9-recommended", "cloudflare-standard", "dns0-zero", "controld-unfiltered"],
    body: `
      <h2>What makes a DNS resolver private</h2>
      <p>Look for four technical properties: an audited no-IP-logging policy, encrypted transport (DoH or DoT), QNAME minimization so upstream root servers see less of your query, and EDNS Client Subnet disabled so your approximate IP subnet is not forwarded.</p>
      <h2>Verify your setup</h2>
      <p>Run <code>dns-optimizer test-privacy</code> to verify which resolver answers your system queries and confirm DNSSEC validation is active.</p>
    `
  },
  {
    slug: "ad-blocking",
    title: "Ad Blocking DNS: Block Ads and Trackers at the Network Level",
    description: "Compare free ad-blocking DNS servers including AdGuard DNS, Mullvad Adblock, Control D, and NextDNS. Test real domain sinkhole filtering in Rust.",
    h1: "Ad blocking DNS",
    lead: "Filter advertising, analytics, and telemetry domains across every app and smart TV on your network.",
    profile: "adblock",
    showBench: true,
    cli: "dns-optimizer bench --profile adblock\ndns-optimizer test-adblock --server 94.140.14.14",
    serverIds: ["adguard-default", "mullvad-adblock", "controld-ads", "nextdns", "rethinkdns"],
    body: `
      <h2>How DNS-level ad blocking works</h2>
      <p>When an app tries to load a tracker or ad network domain, a filtering resolver returns <code>0.0.0.0</code>, <code>::</code>, or <code>NXDOMAIN</code> instead of the real IP address. The connection never starts.</p>
      <h2>Limitations to know</h2>
      <p>DNS blocking cannot remove ads served from the exact same hostname as the main content, such as first-party video ads. Combine it with a browser content blocker for full coverage.</p>
    `
  },
  {
    slug: "family",
    title: "Family Safe DNS: Block Adult Content and Malware Automatically",
    description: "Set up free family-safe DNS servers on home routers, PCs, and phones. Compare Cloudflare for Families, AdGuard Family, CleanBrowsing, and OpenDNS FamilyShield.",
    h1: "Family safe DNS",
    lead: "Filter adult websites, malware domains, and enforce SafeSearch across every device on your Wi-Fi.",
    profile: "family",
    showBench: true,
    cli: "dns-optimizer bench --profile family",
    serverIds: ["cloudflare-family", "adguard-family", "cleanbrowsing-family", "opendns-familyshield", "controld-family"],
    body: `
      <h2>Router-level vs device-level protection</h2>
      <p>Configuring a family DNS server on your home router protects laptops, tablets, game consoles, and smart TVs at once. For phones that leave home Wi-Fi, set the matching Private DNS hostname under Android or iOS settings.</p>
      <h2>SafeSearch enforcement</h2>
      <p>AdGuard Family and CleanBrowsing Family also force SafeSearch on major search engines and restricted mode on supported video platforms.</p>
    `
  },
  {
    slug: "leak-test",
    title: "DNS Leak Test: Check If Your Queries Escape to Your ISP",
    description: "Understand how DNS leaks happen on VPNs and home networks, and run the Rust privacy and DNSSEC validation test from your machine.",
    h1: "DNS leak test",
    lead: "Check whether your system sends DNS lookups to your ISP instead of your chosen encrypted resolver.",
    profile: "privacy",
    showBench: false,
    cli: "dns-optimizer test-privacy\ndns-optimizer status",
    serverIds: ["mullvad-standard", "quad9-recommended", "cloudflare-standard", "dns0-zero"],
    body: `
      <h2>How DNS leaks happen</h2>
      <p>Even when you configure a VPN or custom IPv4 DNS, operating systems sometimes fall back to DHCP-assigned ISP servers over IPv6 or secondary interfaces. Plaintext UDP/53 queries can also be transparently intercepted by some ISP routers.</p>
      <h2>How to fix a leak</h2>
      <p>Enable DNS-over-HTTPS (DoH) or DNS-over-TLS (DoT) so middleboxes cannot read or hijack port 53 traffic, and verify both IPv4 and IPv6 DNS assignments on your active adapter.</p>
    `
  },
  {
    slug: "checker",
    title: "DNS Checker: Query Live A, AAAA, MX, TXT, and NS Records",
    description: "Check live DNS propagation across Cloudflare, Google DNS, and Quad9 right from your browser using DNS-over-HTTPS.",
    h1: "DNS checker",
    lead: "Inspect live DNS records across Cloudflare, Google DNS, and Quad9 from your browser.",
    profile: "speed",
    showBench: false,
    showChecker: true,
    cli: "dns-optimizer bench --profile speed --protocol doh",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended"],
    body: `
      <h2>Checking DNS propagation</h2>
      <p>After updating nameservers or domain records, different recursive resolvers expire cached TTL entries at different times. Querying three independent global Anycast networks shows whether your new records are live.</p>
    `
  },
  {
    slug: "cloudflare-vs-google",
    title: "Cloudflare DNS vs Google DNS: 1.1.1.1 vs 8.8.8.8 Comparison",
    description: "Side-by-side comparison of Cloudflare 1.1.1.1 and Google Public DNS 8.8.8.8 covering latency, EDNS Client Subnet, privacy audits, and filtering.",
    h1: "Cloudflare vs Google DNS",
    lead: "Compare 1.1.1.1 and 8.8.8.8 on privacy, EDNS Client Subnet, speed, and family filtering.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --runs 5",
    serverIds: ["cloudflare-standard", "cloudflare-malware", "cloudflare-family", "google-public-dns"],
    body: `
      <h2>Key technical differences</h2>
      <p>Cloudflare (1.1.1.1) omits EDNS Client Subnet to avoid exposing your IP prefix to authoritative nameservers and undergoes annual KPMG privacy audits. Google Public DNS (8.8.8.8) sends truncated EDNS Client Subnet data to improve CDN geolocation for some streaming providers.</p>
      <h2>Built-in filtering options</h2>
      <p>Cloudflare provides dedicated filtering endpoints (1.1.1.2 for malware, 1.1.1.3 for malware and adult content). Google 8.8.8.8 is strictly unfiltered.</p>
    `
  },
  {
    slug: "countries/usa",
    title: "Best DNS Servers USA: Low-Latency Resolvers Across US Networks",
    description: "Compare the fastest and most reliable public DNS servers in the United States. Test Anycast latency from your US ISP using Rust or browser DoH.",
    h1: "Best DNS servers in the USA",
    lead: "Anycast resolvers with dense US peering across major fiber, cable, and wireless ISPs.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --top 10",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "opendns-home", "controld-unfiltered", "nextdns"],
    body: `
      <h2>US peering and ISP notes</h2>
      <p>Cloudflare, Google, Quad9, and OpenDNS operate dozens of Anycast points of presence across US metros. Cable and fiber subscribers often see 5 to 20 ms median query times compared to congested ISP defaults.</p>
      <h2>Privacy under US jurisdiction</h2>
      <p>If you prefer a non-US jurisdiction with strict no-log commitments, test Quad9 (Switzerland) or Mullvad DNS (Sweden), both of which maintain fast US Anycast nodes.</p>
    `
  },
  {
    slug: "countries/uk",
    title: "Best DNS Servers UK: Fast Resolvers for British Broadband and Fiber",
    description: "Find the fastest DNS servers in the United Kingdom for BT, Virgin Media, Sky, and fiber ISPs. Run a live DoH or Rust UDP/53 benchmark.",
    h1: "Best DNS servers in the UK",
    lead: "Resolvers with strong LINX London and Manchester peering for UK broadband and mobile networks.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --top 10",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "mullvad-standard", "adguard-default", "dns0-zero"],
    body: `
      <h2>UK peering notes</h2>
      <p>Resolvers peering directly at LINX in London and Manchester typically answer UK fiber connections in under 15 ms. European privacy options such as Quad9, Mullvad, and dns0.eu also maintain low-latency UK nodes.</p>
      <h2>Encrypted DNS on UK ISPs</h2>
      <p>Using DoH or DoT prevents plaintext port 53 inspection and ensures consistent resolution across home broadband and 5G networks.</p>
    `
  },
  {
    slug: "countries/india",
    title: "Best DNS Servers India: Fast Anycast Resolvers for Jio, Airtel, and ACT",
    description: "Compare low-latency public DNS servers in India across Mumbai, Delhi, Bengaluru, and Chennai peering hubs. Run a live test from your connection.",
    h1: "Best DNS servers in India",
    lead: "Anycast resolvers with local points of presence in Mumbai, Chennai, Bengaluru, and New Delhi.",
    profile: "speed",
    showBench: true,
    cli: "dns-optimizer bench --profile speed --top 10",
    serverIds: ["cloudflare-standard", "google-public-dns", "quad9-recommended", "adguard-default", "nextdns", "controld-unfiltered"],
    body: `
      <h2>Indian ISP routing notes</h2>
      <p>Routing quality varies widely between JioFiber, Airtel Xstream, ACT Fibernet, and BSNL. Cloudflare, Google, and Quad9 maintain local Indian Anycast nodes that avoid international detours to Singapore or Europe.</p>
      <h2>Use DoH or DoT on mobile networks</h2>
      <p>Some Indian mobile and broadband networks intercept plaintext UDP port 53 requests. Enabling Android Private DNS (DoT) or DoH in the Rust app bypasses port 53 interference.</p>
    `
  }
];

function renderPage(p) {
  const depth = p.slug.split("/").length;
  const base = "../".repeat(depth);
  const canonical = `${SITE_URL}/${p.slug}/`;
  const list = byIds(p.serverIds);

  const checkerHtml = p.showChecker
    ? `<section class="section section--alt">
        <div class="wrap">
          <div class="section-head">
            <h2>Live DoH record lookup</h2>
          </div>
          <form class="form-row" data-dns-checker>
            <input class="input" type="text" name="domain" value="example.com" aria-label="Domain name" required>
            <select class="input" name="rtype" aria-label="Record type">
              <option value="A">A</option>
              <option value="AAAA">AAAA</option>
              <option value="MX">MX</option>
              <option value="TXT">TXT</option>
              <option value="NS">NS</option>
            </select>
            <button type="submit" class="btn btn--primary">Check DNS</button>
          </form>
          <pre class="cli-block" data-checker-output>Enter a domain name and click Check DNS to query Cloudflare, Google, and Quad9.</pre>
        </div>
      </section>`
    : "";

  const benchHtml = p.showBench
    ? renderBenchWidget(p.profile, `${p.h1} live DoH test`)
    : `<div class="box">
        <h3>Run native Rust CLI</h3>
        <pre class="cli-block"><code>${p.cli}</code></pre>
        <a class="btn btn--primary btn--sm" href="${REPO_URL}">Get Rust app on GitHub</a>
      </div>`;

  return `<!DOCTYPE html>
<html lang="en" data-base="${base}">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${p.title} | DNS Optimizer</title>
  <meta name="description" content="${p.description}">
  <link rel="canonical" href="${canonical}">
  <meta property="og:type" content="article">
  <meta property="og:title" content="${p.title}">
  <meta property="og:description" content="${p.description}">
  <meta property="og:url" content="${canonical}">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Chivo+Mono:wght@400;500;800&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="${base}site.css">
  <script type="application/ld+json">
  {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    "itemListElement": [
      { "@type": "ListItem", "position": 1, "name": "DNS Optimizer", "item": "${SITE_URL}/" },
      { "@type": "ListItem", "position": 2, "name": "${p.h1}", "item": "${canonical}" }
    ]
  }
  </script>
</head>
<body>
  <header class="site-nav">
    <div class="wrap nav-inner">
      <a class="brand" href="${base}">
        <span>DNS Optimizer</span>
        <span class="brand-ver">v1.0</span>
      </a>
      <nav aria-label="Primary navigation">
        <ul class="nav-links">
          <li><a href="${base}dns-speed-test/">Speed test</a></li>
          <li><a href="${base}dns-changer/">DNS changer</a></li>
          <li><a href="${base}privacy/">Privacy</a></li>
          <li><a href="${base}ad-blocking/">Ad blocking</a></li>
          <li><a href="${base}checker/">Checker</a></li>
          <li><a href="${REPO_URL}">GitHub</a></li>
        </ul>
      </nav>
    </div>
  </header>

  <main>
    <section class="hero">
      <div class="wrap hero-grid">
        <div>
          <h1>${p.h1}</h1>
          <p class="hero-lead">${p.lead}</p>
          <div class="hero-actions">
            <a class="btn btn--primary" href="${REPO_URL}">Download Rust app</a>
            <a class="btn" href="${base}">All DNS guides</a>
          </div>
        </div>
        ${benchHtml}
      </div>
    </section>

    ${checkerHtml}

    <section class="section">
      <div class="wrap">
        <div class="section-head">
          <h2>Recommended DNS servers</h2>
        </div>
        ${renderServerTable(list)}
      </div>
    </section>

    <section class="section section--alt">
      <div class="wrap">
        ${p.body}
        <h2>Rust CLI command</h2>
        <pre class="cli-block"><code>${p.cli}</code></pre>
      </div>
    </section>
  </main>

  <footer class="site-footer">
    <div class="wrap">
      <p class="footer-statement">Free MIT-licensed Rust software for real DNS benchmarks and safe resolver switching.</p>
      <div class="footer-links">
        <a href="${base}">Home</a>
        <a href="${base}dns-speed-test/">Speed test</a>
        <a href="${base}dns-changer/">DNS changer</a>
        <a href="${base}leak-test/">Leak test</a>
        <a href="${base}checker/">Checker</a>
        <a href="${REPO_URL}">GitHub</a>
      </div>
      <p class="muted">Changing DNS speeds up domain lookups, not active in-game ping.</p>
    </div>
  </footer>

  <script src="${base}app.js"></script>
</body>
</html>
`;
}

const sitemapUrls = [`${SITE_URL}/`];

for (const page of PAGES) {
  const outDir = path.join(DOCS, page.slug);
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, "index.html"), renderPage(page), "utf8");
  sitemapUrls.push(`${SITE_URL}/${page.slug}/`);
}

// Write sitemap.xml
const today = "2026-11-25";
const sitemapXml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${sitemapUrls
  .map(
    (u, i) => `  <url>
    <loc>${u}</loc>
    <lastmod>${today}</lastmod>
    <priority>${i === 0 ? "1.0" : "0.8"}</priority>
  </url>`
  )
  .join("\n")}
</urlset>
`;
fs.writeFileSync(path.join(DOCS, "sitemap.xml"), sitemapXml, "utf8");

// Write robots.txt
const robotsTxt = `User-agent: *
Allow: /

Sitemap: ${SITE_URL}/sitemap.xml
`;
fs.writeFileSync(path.join(DOCS, "robots.txt"), robotsTxt, "utf8");

// Write llms.txt
const llmsTxt = `# DNS Optimizer

> Open-source Rust DNS benchmark, speed test, and cross-platform DNS changer for Windows, macOS, Linux, Android, and Web.

- Repository: ${REPO_URL}
- Website: ${SITE_URL}/
- License: MIT
- Catalog JSON: ${SITE_URL}/dns_servers.json

## Core capabilities
- Real RFC 1035 DNS lookup latency benchmark (UDP/53, TCP/53, DoT, DoH) rather than ICMP ping
- One-click cross-platform system DNS changer with automatic backup and restore
- Smart profiles: Maximum speed, Privacy, Ad blocking, Family protection
- DNS Privacy & DNSSEC validation test (dnssec-failed.org SERVFAIL check)
- Ad-blocking sinkhole verification test
- Auto Switch health monitor and local JSONL speed history
- Android Private DNS (DoT) guide and C-ABI / Tauri v2 mobile support

## Pages
${sitemapUrls.map((u) => `- ${u}`).join("\n")}
`;
fs.writeFileSync(path.join(DOCS, "llms.txt"), llmsTxt, "utf8");

console.log(`Generated ${PAGES.length} subpages + sitemap.xml (${sitemapUrls.length} URLs), robots.txt, llms.txt`);
