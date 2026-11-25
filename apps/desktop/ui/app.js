const invoke = window.__TAURI__?.core?.invoke;

let activeProfile = "speed";
let activeProto = "udp";
let lastRecommended = null;

function setStatus(msg) {
  document.getElementById("bench-status").textContent = msg;
}

async function refreshSystemStatus() {
  const el = document.getElementById("system-status");
  if (!invoke) {
    el.textContent = "Preview mode (launch via Tauri for OS DNS control)";
    return;
  }
  try {
    const st = await invoke("system_status");
    const dns = st.active_servers.length ? st.active_servers.join(", ") : "DHCP default";
    el.textContent = `${st.os} (${st.interface}): ${dns}`;
  } catch (e) {
    el.textContent = `Status error: ${e}`;
  }
}

document.querySelectorAll(".goal-group .cell").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".goal-group .cell").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    activeProfile = btn.dataset.profile;
  });
});

document.querySelectorAll(".proto-group .cell").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".proto-group .cell").forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    activeProto = btn.dataset.proto;
  });
});

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    tab.classList.add("active");
    const name = tab.dataset.tab;
    ["bench", "privacy", "adblock", "history"].forEach((p) => {
      document.getElementById(`panel-${p}`).hidden = p !== name;
    });
  });
});

document.getElementById("btn-start-bench").addEventListener("click", async () => {
  if (!invoke) {
    setStatus("Run inside the Tauri desktop or Android app to execute native UDP/DoT/DoH benchmarks.");
    return;
  }
  setStatus(`Testing ${activeProfile} resolvers over ${activeProto.toUpperCase()}...`);
  try {
    const report = await invoke("start_benchmark", { profile: activeProfile, protocol: activeProto });
    const tbody = document.getElementById("bench-tbody");
    tbody.innerHTML = "";

    for (const row of report.results.slice(0, 15)) {
      const tr = document.createElement("tr");
      const med = row.queries_ok > 0 ? `${row.median_ms.toFixed(1)} ms` : "timeout";
      const min = row.queries_ok > 0 ? `${row.min_ms.toFixed(1)} ms` : "-";
      tr.innerHTML = `
        <td>${row.name}</td>
        <td>${row.primary_ip}</td>
        <td class="ms">${med}</td>
        <td>${min}</td>
        <td>${Math.round(row.success_rate * 100)}%</td>
        <td>${row.is_current_system_dns ? "Active" : `<button class="btn btn-outline btn-sm" data-apply="${row.server_id}">Apply</button>`}</td>
      `;
      tbody.appendChild(tr);
    }

    tbody.querySelectorAll("[data-apply]").forEach((btn) => {
      btn.addEventListener("click", () => applyServer(btn.dataset.apply));
    });

    if (report.recommended) {
      lastRecommended = report.recommended;
      document.getElementById("recommended-plate").hidden = false;
      document.getElementById("rec-name").textContent = `${report.recommended.name} (${report.recommended.primary_ip})`;
      document.getElementById("rec-ms").textContent = `${report.recommended.median_ms.toFixed(1)} ms`;
      document.getElementById("rec-meta").textContent = `Best score for ${activeProfile} profile over ${activeProto.toUpperCase()}`;
    }
    setStatus(`Tested ${report.results.length} servers.`);
  } catch (e) {
    setStatus(`Benchmark error: ${e}`);
  }
});

async function applyServer(serverId) {
  if (!invoke) return;
  try {
    const res = await invoke("apply_dns_server", { serverId, dryRun: false });
    setStatus(`Applied ${res.applied_servers.join(", ")} to ${res.interface}. Backup saved.`);
    refreshSystemStatus();
  } catch (e) {
    setStatus(`Apply failed (run with admin privileges): ${e}`);
  }
}

document.getElementById("btn-apply-rec").addEventListener("click", () => {
  if (lastRecommended) applyServer(lastRecommended.server_id);
});

document.getElementById("btn-restore").addEventListener("click", async () => {
  if (!invoke) return;
  try {
    const res = await invoke("restore_system_dns", { dryRun: false });
    setStatus(`Restored DNS on ${res.interface}.`);
    refreshSystemStatus();
  } catch (e) {
    setStatus(`Restore failed: ${e}`);
  }
});

document.getElementById("btn-run-privacy").addEventListener("click", async () => {
  if (!invoke) return;
  const out = document.getElementById("privacy-output");
  out.textContent = "Running privacy and DNSSEC check...";
  try {
    const rep = await invoke("privacy_test");
    out.textContent = [
      `Active resolvers: ${rep.active_servers.join(", ") || "None detected"}`,
      `Matched known providers: ${rep.matched_known_providers.join(", ") || "None (ISP / router)"}`,
      `DNSSEC validation: ${rep.dnssec_validated ? "PASS" : "FAIL"} (${rep.dnssec_probe_rcode})`,
      `Leak risk: ${rep.leak_risk}`,
      `Recommendation: ${rep.recommendation}`
    ].join("\n");
  } catch (e) {
    out.textContent = `Error: ${e}`;
  }
});

document.getElementById("btn-run-adblock").addEventListener("click", async () => {
  if (!invoke) return;
  const ip = document.getElementById("adblock-ip").value.trim();
  const out = document.getElementById("adblock-output");
  out.textContent = `Testing ad-blocking on ${ip}...`;
  try {
    const rep = await invoke("adblock_test", { serverIp: ip });
    const lines = [
      `Blocked ${rep.blocked_count}/${rep.total_tested} (${rep.block_rate_percent}%) on ${rep.server_ip}`,
      "",
      ...rep.checks.map((c) => `[${c.blocked ? "BLOCKED" : "ALLOWED"}] ${c.domain} (${c.rcode})`)
    ];
    out.textContent = lines.join("\n");
  } catch (e) {
    out.textContent = `Error: ${e}`;
  }
});

document.getElementById("btn-refresh-history").addEventListener("click", async () => {
  if (!invoke) return;
  const out = document.getElementById("history-output");
  try {
    const items = await invoke("benchmark_history", { limit: 15 });
    if (!items.length) {
      out.textContent = "No benchmark history recorded yet.";
      return;
    }
    out.textContent = items
      .map((e) => `${e.timestamp.slice(0, 19).replace("T", " ")} | ${e.profile} | ${e.winner_name} (${e.winner_median_ms} ms)`)
      .join("\n");
  } catch (e) {
    out.textContent = `Error: ${e}`;
  }
});

document.getElementById("btn-health-check").addEventListener("click", async () => {
  if (!invoke) return;
  const out = document.getElementById("history-output");
  try {
    const tick = await invoke("health_check", { profile: activeProfile, autoSwitch: true });
    out.textContent = `Health check: active=${tick.active_ip} healthy=${tick.healthy} latency=${tick.latency_ms ?? "-"} ms switched=${tick.switched_to ?? "no"}`;
  } catch (e) {
    out.textContent = `Error: ${e}`;
  }
});

refreshSystemStatus();
