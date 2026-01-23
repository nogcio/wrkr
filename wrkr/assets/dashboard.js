(() => {
  const statusEl = document.getElementById("status");
  const rowsEl = document.getElementById("rows");
  const errorsEl = document.getElementById("errors");

  const state = new Map(); // scenario -> row
  const MAX_POINTS = Number(document.body?.dataset?.maxPoints ?? "300"); // client-side cap

  const palette = [
    "#3b82f6", "#ef4444", "#22c55e", "#a855f7", "#f59e0b",
    "#06b6d4", "#f97316", "#84cc16", "#e11d48", "#14b8a6",
  ];
  const colorByScenario = new Map();
  function colorForScenario(scenario) {
    if (colorByScenario.has(scenario)) return colorByScenario.get(scenario);
    const c = palette[colorByScenario.size % palette.length];
    colorByScenario.set(scenario, c);
    return c;
  }

  function makeChart(canvasId, yLabel) {
    const el = document.getElementById(canvasId);
    if (!el || typeof Chart === "undefined") return null;
    const ctx = el.getContext("2d");
    return new Chart(ctx, {
      type: "line",
      data: { datasets: [] },
      options: {
        animation: false,
        responsive: true,
        maintainAspectRatio: false,
        parsing: false,
        interaction: { mode: "nearest", intersect: false },
        plugins: { legend: { display: true, labels: { boxWidth: 10 } } },
        scales: {
          x: { type: "linear", title: { display: true, text: "elapsed (s)" } },
          y: { title: { display: true, text: yLabel } },
        },
        elements: { point: { radius: 0 } },
      },
    });
  }

  const chartRps = makeChart("chartRps", "rps");
  const chartLat = makeChart("chartLat", "ms");
  const chartErrRate = makeChart("chartErrRate", "%");
  const chartVus = makeChart("chartVus", "vus");
  const chartIters = makeChart("chartIters", "iters/s");
  const chartNet = makeChart("chartNet", "bytes/s");

  function ensureDataset(chart, label, opts) {
    if (!chart) return null;
    let ds = chart.data.datasets.find((d) => d.label === label);
    if (ds) return ds;
    const color = opts?.color ?? "#3b82f6";
    ds = {
      label,
      data: [],
      borderColor: color,
      backgroundColor: color,
      borderWidth: 1.5,
      tension: 0.2,
      borderDash: opts?.dash ?? [],
    };
    chart.data.datasets.push(ds);
    return ds;
  }

  function pushPoint(chart, label, x, y, opts) {
    if (!chart) return;
    if (y == null || !Number.isFinite(y)) return;
    const ds = ensureDataset(chart, label, opts);
    if (!ds) return;
    ds.data.push({ x, y });
    if (ds.data.length > MAX_POINTS) ds.data.shift();
  }

  function fmtInt(v) { return (v ?? 0).toLocaleString("en-US"); }
  function fmtFloat(v) { return (v ?? 0).toFixed(1); }
  function fmtOptFloat(v) { return v == null ? "-" : v.toFixed(1); }
  function fmtDur(secs) { return secs == null ? "-" : (secs + "s"); }
  function fmtVus(cur, max) { return max == null ? fmtInt(cur) : (fmtInt(cur) + "/" + fmtInt(max)); }

  function render() {
    const scenarios = Array.from(state.keys()).sort();
    rowsEl.innerHTML = "";
    for (const scenario of scenarios) {
      const r = state.get(scenario);
      const tr = document.createElement("tr");
      tr.innerHTML = `
        <td class="mono">${scenario}</td>
        <td class="mono">${r.exec ?? ""}</td>
        <td class="right mono">${fmtDur(r.elapsedSecs)}</td>
        <td class="right mono">${fmtVus(r.vusCurrent, r.vusMax)}</td>
        <td class="right mono">${fmtFloat(r.rpsNow)}</td>
        <td class="right mono">${fmtOptFloat(r.latencyP95MsNow)}</td>
        <td class="right mono">${fmtFloat((r.errorRateNow ?? 0) * 100)}</td>
        <td class="right mono">${fmtFloat(r.iterationsPerSecNow ?? 0)}</td>
        <td class="right mono">${fmtInt(r.requestsTotal)}</td>
        <td class="right mono">${fmtInt(r.failedRequestsTotal)}</td>
        <td class="right mono">${fmtInt(r.checksFailedTotal)}</td>
        <td class="right mono">${fmtInt(r.bytesReceivedPerSecNow)}</td>
        <td class="right mono">${fmtInt(r.bytesSentPerSecNow)}</td>
        <td class="right mono">${fmtInt(r.droppedIterationsTotal)}</td>
      `;
      rowsEl.appendChild(tr);
    }

    if (errorsEl) {
      errorsEl.innerHTML = "";
      for (const scenario of scenarios) {
        const r = state.get(scenario);
        const errs = r?.errorsNow ?? {};
        const items = Object.entries(errs)
          .filter(([, v]) => (v ?? 0) > 0)
          .sort((a, b) => (b[1] ?? 0) - (a[1] ?? 0))
          .slice(0, 10);
        for (const [k, v] of items) {
          const tr = document.createElement("tr");
          tr.innerHTML = `
            <td class="mono">${scenario}</td>
            <td class="mono">${k}</td>
            <td class="right mono">${fmtInt(v)}</td>
          `;
          errorsEl.appendChild(tr);
        }
      }
    }
  }

  function chartsUpdate() {
    if (chartRps) chartRps.update("none");
    if (chartLat) chartLat.update("none");
    if (chartErrRate) chartErrRate.update("none");
    if (chartVus) chartVus.update("none");
    if (chartIters) chartIters.update("none");
    if (chartNet) chartNet.update("none");
  }

  function setSeries(chart, label, points, opts) {
    if (!chart) return;
    const ds = ensureDataset(chart, label, opts);
    if (!ds) return;
    ds.data = (points ?? []).slice(-MAX_POINTS);
  }

  function clearCharts() {
    if (chartRps) chartRps.data.datasets = [];
    if (chartLat) chartLat.data.datasets = [];
    if (chartErrRate) chartErrRate.data.datasets = [];
    if (chartVus) chartVus.data.datasets = [];
    if (chartIters) chartIters.data.datasets = [];
    if (chartNet) chartNet.data.datasets = [];
    colorByScenario.clear();
  }

  const metricDashes = {
    p50: [],
    p90: [6, 4],
    p95: [2, 3],
    p99: [10, 4, 2, 4],
    rx: [],
    tx: [6, 4],
  };

  function applySnapshot(msg) {
    state.clear();
    clearCharts();
    for (const [scenario, snap] of Object.entries(msg.scenarios ?? {})) {
      const latest = snap.latest ?? snap;
      state.set(scenario, latest);
      const color = colorForScenario(scenario);
      if (snap.series) {
        setSeries(chartRps, scenario, snap.series.rps, { color });
        setSeries(chartLat, scenario + " p50", snap.series.latP50, { color, dash: metricDashes.p50 });
        setSeries(chartLat, scenario + " p90", snap.series.latP90, { color, dash: metricDashes.p90 });
        setSeries(chartLat, scenario + " p95", snap.series.latP95, { color, dash: metricDashes.p95 });
        setSeries(chartLat, scenario + " p99", snap.series.latP99, { color, dash: metricDashes.p99 });
        setSeries(chartErrRate, scenario, snap.series.errorRate, { color });
        setSeries(chartVus, scenario, snap.series.vus, { color });
        setSeries(chartIters, scenario, snap.series.iters, { color });
        setSeries(chartNet, scenario + " rx", snap.series.rx, { color, dash: metricDashes.rx });
        setSeries(chartNet, scenario + " tx", snap.series.tx, { color, dash: metricDashes.tx });
      } else {
        pushPoint(chartRps, scenario, latest.elapsedSecs, latest.rpsNow, { color });
        pushPoint(chartLat, scenario + " p50", latest.elapsedSecs, latest.latencyP50MsNow, { color, dash: metricDashes.p50 });
        pushPoint(chartLat, scenario + " p90", latest.elapsedSecs, latest.latencyP90MsNow, { color, dash: metricDashes.p90 });
        pushPoint(chartLat, scenario + " p95", latest.elapsedSecs, latest.latencyP95MsNow, { color, dash: metricDashes.p95 });
        pushPoint(chartLat, scenario + " p99", latest.elapsedSecs, latest.latencyP99MsNow, { color, dash: metricDashes.p99 });
        pushPoint(chartErrRate, scenario, latest.elapsedSecs, (latest.errorRateNow ?? 0) * 100, { color });
        pushPoint(chartVus, scenario, latest.elapsedSecs, latest.vusCurrent, { color });
        pushPoint(chartIters, scenario, latest.elapsedSecs, latest.iterationsPerSecNow, { color });
        pushPoint(chartNet, scenario + " rx", latest.elapsedSecs, latest.bytesReceivedPerSecNow, { color, dash: metricDashes.rx });
        pushPoint(chartNet, scenario + " tx", latest.elapsedSecs, latest.bytesSentPerSecNow, { color, dash: metricDashes.tx });
      }
    }
    render();
    chartsUpdate();
  }

  function wsUrl() {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    return proto + "//" + location.host + "/ws";
  }

  function setStatus(text) {
    if (!statusEl) return;
    statusEl.textContent = text;
  }

  function setInitialStatus(mode) {
    if (!chartRps || !chartLat || !chartErrRate || !chartVus || !chartIters || !chartNet) {
      setStatus(`${mode} (Chart.js unavailable - showing table only)`);
      return;
    }
    setStatus(mode);
  }

  function startOffline() {
    const el = document.getElementById("wrkrSnapshot");
    if (!el) return false;
    let msg;
    try { msg = JSON.parse(el.textContent ?? ""); } catch { msg = null; }
    if (!msg || msg.type !== "snapshot") {
      setStatus("offline (invalid snapshot)");
      return true;
    }
    setInitialStatus("offline");
    applySnapshot(msg);
    return true;
  }

  function startLive() {
    setInitialStatus("connecting…");
    const ws = new WebSocket(wsUrl());
    ws.onopen = () => setInitialStatus("connected");
    ws.onclose = () => {
      setStatus("disconnected");
      statusEl?.classList?.add("mono");
    };
    ws.onerror = () => setStatus("error");
    ws.onmessage = (ev) => {
      let msg;
      try { msg = JSON.parse(ev.data); } catch { return; }
      if (msg.type === "snapshot") {
        applySnapshot(msg);
        return;
      }
      if (msg.type === "update") {
        if (msg.scenario && msg.data) {
          state.set(msg.scenario, msg.data);
          const r = msg.data;
          const color = colorForScenario(msg.scenario);
          pushPoint(chartRps, msg.scenario, r.elapsedSecs, r.rpsNow, { color });
          pushPoint(chartLat, msg.scenario + " p50", r.elapsedSecs, r.latencyP50MsNow, { color, dash: metricDashes.p50 });
          pushPoint(chartLat, msg.scenario + " p90", r.elapsedSecs, r.latencyP90MsNow, { color, dash: metricDashes.p90 });
          pushPoint(chartLat, msg.scenario + " p95", r.elapsedSecs, r.latencyP95MsNow, { color, dash: metricDashes.p95 });
          pushPoint(chartLat, msg.scenario + " p99", r.elapsedSecs, r.latencyP99MsNow, { color, dash: metricDashes.p99 });
          pushPoint(chartErrRate, msg.scenario, r.elapsedSecs, (r.errorRateNow ?? 0) * 100, { color });
          pushPoint(chartVus, msg.scenario, r.elapsedSecs, r.vusCurrent, { color });
          pushPoint(chartIters, msg.scenario, r.elapsedSecs, r.iterationsPerSecNow, { color });
          pushPoint(chartNet, msg.scenario + " rx", r.elapsedSecs, r.bytesReceivedPerSecNow, { color, dash: metricDashes.rx });
          pushPoint(chartNet, msg.scenario + " tx", r.elapsedSecs, r.bytesSentPerSecNow, { color, dash: metricDashes.tx });
          render();
          chartsUpdate();
        }
        return;
      }
      if (msg.type === "done") {
        setStatus("done (run finished)");
      }
    };
  }

  if (!startOffline()) {
    startLive();
  }
})();

