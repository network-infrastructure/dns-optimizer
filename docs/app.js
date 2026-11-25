(() => {
  const BASE = document.documentElement.dataset.base || "./";
  const CATALOG_URL = `${BASE}dns_servers.json`;

  const DEMO_ROWS = {
    speed: [
      { id: "cloudflare-standard", name: "Cloudflare", ip: "1.1.1.1", ms: "8 ms", note: "Demo value" },
      { id: "google-public-dns", name: "Google DNS", ip: "8.8.8.8", ms: "14 ms", note: "Demo value" },
      { id: "quad9-recommended", name: "Quad9", ip: "9.9.9.9", ms: "19 ms", note: "Demo value" },
      { id: "adguard-default", name: "AdGuard DNS", ip: "94.140.14.14", ms: "23 ms", note: "Demo value" },
      { id: "current-isp", name: "Current DNS", ip: "ISP Default", ms: "42 ms", note: "Demo value" }
    ],
    privacy: [
      { id: "mullvad-standard", name: "Mullvad DNS", ip: "194.242.2.2", ms: "18 ms", note: "Demo value" },
      { id: "quad9-recommended", name: "Quad9", ip: "9.9.9.9", ms: "19 ms", note: "Demo value" },
      { id: "cloudflare-standard", name: "Cloudflare", ip: "1.1.1.1", ms: "8 ms", note: "Demo value" },
      { id: "dns0-zero", name: "dns0.eu ZERO", ip: "193.110.81.9", ms: "22 ms", note: "Demo value" }
    ],
    adblock: [
      { id: "adguard-default", name: "AdGuard DNS", ip: "94.140.14.14", ms: "23 ms", note: "Demo value" },
      { id: "mullvad-adblock", name: "Mullvad Adblock", ip: "194.242.2.3", ms: "21 ms", note: "Demo value" },
      { id: "controld-ads", name: "Control D Ads", ip: "76.76.2.2", ms: "17 ms", note: "Demo value" },
      { id: "nextdns", name: "NextDNS", ip: "45.90.28.0", ms: "19 ms", note: "Demo value" }
    ],
    family: [
      { id: "cloudflare-family", name: "Cloudflare Family", ip: "1.1.1.3", ms: "9 ms", note: "Demo value" },
      { id: "adguard-family", name: "AdGuard Family", ip: "94.140.14.15", ms: "24 ms", note: "Demo value" },
      { id: "cleanbrowsing-family", name: "CleanBrowsing Family", ip: "185.228.168.168", ms: "27 ms", note: "Demo value" },
      { id: "opendns-familyshield", name: "OpenDNS FamilyShield", ip: "208.67.222.123", ms: "25 ms", note: "Demo value" }
    ]
  };

  let catalogCache = null;
  let activeProfile = "speed";

  async function loadCatalog() {
    if (catalogCache) return catalogCache;
    try {
      const resp = await fetch(CATALOG_URL);
      const data = await resp.json();
      catalogCache = data.servers || [];
      return catalogCache;
    } catch {
      return [];
    }
  }

  function matchesProfile(server, profile) {
    const filtering = (server.filtering || []).join(" ").toLowerCase();
    if (profile === "privacy") return (server.privacy_score || 0) >= 4 && server.dnssec;
    if (profile === "adblock") return filtering.includes("ad") || filtering.includes("tracker");
    if (profile === "family") return filtering.includes("adult") || filtering.includes("family") || filtering.includes("safe search");
    return true;
  }

  function renderRows(container, rows, isLive) {
    container.innerHTML = "";
    rows.forEach((row) => {
      const div = document.createElement("div");
      div.className = "bench-row";
      div.innerHTML = `
        <div class="bench-name">${row.name}</div>
        <div class="bench-ip">${row.ip}</div>
        <div class="bench-ms ${isLive ? "is-live" : ""}">${row.ms}</div>
      `;
      container.appendChild(div);
    });
  }

  async function probeDoh(server) {
    if (!server.probe_url) return null;
    const url = `${server.probe_url}&_t=${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 2500);
    const t0 = performance.now();
    try {
      const resp = await fetch(url, {
        method: "GET",
        headers: { Accept: "application/dns-json" },
        cache: "no-store",
        signal: controller.signal
      });
      clearTimeout(timer);
      if (!resp.ok) return null;
      await resp.text();
      return Math.round((performance.now() - t0) * 10) / 10;
    } catch {
      clearTimeout(timer);
      return null;
    }
  }

  async function runLiveDohTest(boardEl) {
    const rowsEl = boardEl.querySelector("[data-bench-rows]");
    const subEl = boardEl.querySelector("[data-bench-sub]");
    const winnerEl = boardEl.querySelector("[data-bench-winner]");
    const figEl = boardEl.querySelector("[data-bench-figure]");
    const noteEl = boardEl.querySelector("[data-bench-note]");
    const btn = boardEl.querySelector("[data-bench-start]");

    if (btn) btn.disabled = true;
    if (subEl) subEl.textContent = `Testing live DoH resolvers for ${activeProfile}...`;

    const servers = await loadCatalog();
    const candidates = servers
      .filter((s) => s.probe_url && s.probe_cors && matchesProfile(s, activeProfile))
      .slice(0, 8);

    const measured = [];
    for (const srv of candidates) {
      // Warmup + 2 timed probes to reduce cold TLS noise
      await probeDoh(srv);
      const t1 = await probeDoh(srv);
      const t2 = await probeDoh(srv);
      const valid = [t1, t2].filter((v) => typeof v === "number");
      if (valid.length) {
        const median = valid.reduce((a, b) => a + b, 0) / valid.length;
        measured.push({
          id: srv.id,
          name: srv.name,
          ip: (srv.ipv4 && srv.ipv4[0]) || srv.dot || "DoH",
          msNum: Math.round(median),
          ms: `${Math.round(median)} ms`,
          dot: srv.dot || ""
        });
      }
    }

    measured.sort((a, b) => a.msNum - b.msNum);

    if (measured.length) {
      renderRows(rowsEl, measured.slice(0, 6), true);
      const best = measured[0];
      if (subEl) subEl.textContent = "Live browser DoH measurement complete";
      if (winnerEl) winnerEl.textContent = `${best.name} (${best.ip})`;
      if (figEl) figEl.textContent = best.ms;
      if (noteEl) noteEl.textContent = `Lowest DoH response time measured from your browser. Install the Rust app for UDP/53 and one-click apply.`;
    } else {
      if (subEl) subEl.textContent = "Browser blocked cross-origin DoH requests; showing reference demo values";
    }
    if (btn) btn.disabled = false;
  }

  // Initialize any benchmark boards on the page
  document.querySelectorAll("[data-bench-board]").forEach((boardEl) => {
    const rowsEl = boardEl.querySelector("[data-bench-rows]");
    const winnerEl = boardEl.querySelector("[data-bench-winner]");
    const figEl = boardEl.querySelector("[data-bench-figure]");
    const noteEl = boardEl.querySelector("[data-bench-note]");
    const startBtn = boardEl.querySelector("[data-bench-start]");

    const initialProfile = boardEl.dataset.initialProfile || "speed";
    activeProfile = initialProfile;

    function loadDemo(profile) {
      const list = DEMO_ROWS[profile] || DEMO_ROWS.speed;
      renderRows(rowsEl, list, false);
      if (winnerEl) winnerEl.textContent = `${list[0].name} (${list[0].ip})`;
      if (figEl) figEl.textContent = list[0].ms;
      if (noteEl) noteEl.textContent = "Demo values shown before test. Click Start DNS Speed Test for live DoH timing.";
    }

    loadDemo(activeProfile);

    boardEl.querySelectorAll("[data-profile]").forEach((btn) => {
      if (btn.dataset.profile === activeProfile) btn.classList.add("is-active");
      btn.addEventListener("click", () => {
        boardEl.querySelectorAll("[data-profile]").forEach((b) => b.classList.remove("is-active"));
        btn.classList.add("is-active");
        activeProfile = btn.dataset.profile;
        loadDemo(activeProfile);
      });
    });

    if (startBtn) {
      startBtn.addEventListener("click", () => runLiveDohTest(boardEl));
    }
  });

  // Initialize DNS Checker tool on /checker/
  const checkerForm = document.querySelector("[data-dns-checker]");
  if (checkerForm) {
    checkerForm.addEventListener("submit", async (e) => {
      e.preventDefault();
      const domain = checkerForm.querySelector("[name=domain]").value.trim() || "example.com";
      const type = checkerForm.querySelector("[name=rtype]").value || "A";
      const out = document.querySelector("[data-checker-output]");
      out.textContent = `Querying Cloudflare, Google, and Quad9 DoH for ${domain} (${type})...`;

      const resolvers = [
        { name: "Cloudflare (1.1.1.1)", url: `https://cloudflare-dns.com/dns-query?name=${encodeURIComponent(domain)}&type=${type}` },
        { name: "Google DNS (8.8.8.8)", url: `https://dns.google/resolve?name=${encodeURIComponent(domain)}&type=${type}` },
        { name: "Quad9 (9.9.9.9)", url: `https://dns.quad9.net:5053/dns-query?name=${encodeURIComponent(domain)}&type=${type}` }
      ];

      const lines = [];
      for (const r of resolvers) {
        const t0 = performance.now();
        try {
          const resp = await fetch(r.url, { headers: { Accept: "application/dns-json" } });
          const data = await resp.json();
          const ms = Math.round(performance.now() - t0);
          const ans = (data.Answer || []).map((a) => `${a.data} (TTL ${a.TTL}s)`).join(", ") || "No records";
          lines.push(`${r.name}: ${ans} [${ms} ms]`);
        } catch {
          lines.push(`${r.name}: Request blocked or timed out`);
        }
      }
      out.textContent = lines.join("\n");
    });
  }

  // Copy helper for CLI snippets and IPs
  document.querySelectorAll("[data-copy]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(btn.dataset.copy);
        const orig = btn.textContent;
        btn.textContent = "Copied";
        setTimeout(() => { btn.textContent = orig; }, 1400);
      } catch {
        // Ignore clipboard denial
      }
    });
  });
})();
