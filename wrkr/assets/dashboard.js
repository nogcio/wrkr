/* global Chart */

const LIVE_WINDOW_SECONDS = 60;
const LIVE_ANIM_MS = 950;

function cssVar(name, fallback) {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name);
  const s = (v || "").trim();
  return s || fallback;
}

function withAlpha(color, alpha) {
  // Supports hex colors only; for non-hex, just return the input.
  const c = (color || "").trim();
  const m = c.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)(?:\s*,\s*([0-9.]+)\s*)?\)$/i);
  if (m) {
    const r = Number.parseInt(m[1], 10);
    const g = Number.parseInt(m[2], 10);
    const b = Number.parseInt(m[3], 10);
    if ([r, g, b].every((x) => Number.isFinite(x))) {
      return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }
  }
  if (!c.startsWith("#")) return c;
  let hex = c.slice(1);
  if (hex.length === 3) hex = hex.split("").map((x) => x + x).join("");
  if (hex.length !== 6) return c;
  const r = parseInt(hex.slice(0, 2), 16);
  const g = parseInt(hex.slice(2, 4), 16);
  const b = parseInt(hex.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function fmtInt(n) {
  if (n == null || !Number.isFinite(n)) return "—";
  return Math.trunc(n).toLocaleString("en-US");
}

function fmtFloat(n) {
  if (n == null || !Number.isFinite(n)) return "—";
  return n.toFixed(2);
}

function fmtDur(sec) {
  if (sec == null || !Number.isFinite(sec)) return "—";
  if (sec < 60) return `${sec.toFixed(1)}s`;
  const m = Math.floor(sec / 60);
  const s = sec - m * 60;
  return `${m}m ${s.toFixed(0)}s`;
}

function fmtMs(ms) {
  if (ms == null || !Number.isFinite(ms)) return "—";
  if (ms < 1000) return `${ms.toFixed(0)} ms`;
  const sec = ms / 1000;
  if (sec < 60) return `${sec.toFixed(2)} s`;
  return fmtDur(sec);
}

function fmtPct(p) {
  if (p == null || !Number.isFinite(p)) return "—";
  return `${p.toFixed(2)}%`;
}

function fmtPerSec(n) {
  if (n == null || !Number.isFinite(n)) return "—";
  const v = Math.max(0, n);
  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: 2,
    minimumFractionDigits: v > 0 && v < 10 ? 2 : 0,
  }).format(v);
}

function fmtBytesPerSec(n) {
  if (n == null || !Number.isFinite(n)) return "—";
  const v = Math.max(0, n);
  const units = ["B/s", "KiB/s", "MiB/s", "GiB/s"];
  let i = 0;
  let x = v;
  while (x >= 1024 && i < units.length - 1) {
    x /= 1024;
    i += 1;
  }
  const digits = i === 0 ? 0 : x < 10 ? 2 : x < 100 ? 1 : 0;
  return `${x.toFixed(digits)} ${units[i]}`;
}

function fmtRate(n) {
  if (n == null || !Number.isFinite(n)) return "—";
  const v = Math.max(0, n);
  if (v < 1000) return v.toFixed(0);
  if (v < 1_000_000) return `${(v / 1000).toFixed(1)}k`;
  return `${(v / 1_000_000).toFixed(1)}M`;
}

function getSnapshotFromDom() {
  const el = document.getElementById("snapshot");
  if (!el) return null;
  try {
    return JSON.parse(el.textContent);
  } catch {
    return null;
  }
}

function ensureCanvasPixels(canvas) {
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  const w = Math.max(1, Math.floor(rect.width * dpr));
  const h = Math.max(1, Math.floor(rect.height * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  return { w, h, dpr };
}

function makeSpark(id, strokeStyle, fillStyle) {
  const canvas = document.getElementById(id);
  if (!canvas) return null;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  const spark = {
    id,
    canvas,
    ctx,
    strokeStyle,
    fillStyle,
    data: [],
    minX: 0,
    maxX: 0,
    reset() {
      this.data.length = 0;
    },
    push(x, y) {
      this.data.push({ x, y });
    },
    trim(minX) {
      trimSeries(this.data, minX);
    },
    setWindow(minX, maxX) {
      this.minX = minX;
      this.maxX = maxX;
    },
    draw() {
      const { w, h } = ensureCanvasPixels(this.canvas);
      const ctx = this.ctx;
      ctx.clearRect(0, 0, w, h);

      const minX = this.minX;
      const maxX = this.maxX;
      const spanX = maxX - minX;
      if (!Number.isFinite(spanX) || spanX <= 0) return;

      let yMin = Infinity;
      let yMax = -Infinity;
      for (const p of this.data) {
        if (!p || typeof p.x !== "number" || p.x < minX || p.x > maxX) continue;
        if (p.y == null || !Number.isFinite(p.y)) continue;
        yMin = Math.min(yMin, p.y);
        yMax = Math.max(yMax, p.y);
      }
      if (!Number.isFinite(yMin) || !Number.isFinite(yMax)) return;

      if (yMin > 0) yMin = 0;
      let spanY = yMax - yMin;
      if (!Number.isFinite(spanY) || spanY <= 0) spanY = 1;
      const pad = spanY * 0.08;
      yMin -= pad;
      yMax += pad;
      spanY = yMax - yMin;

      const toX = (x) => ((x - minX) / spanX) * w;
      const toY = (y) => h - ((y - yMin) / spanY) * h;

      // Build path (break on nulls).
      let started = false;
      ctx.lineWidth = 1.25;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      ctx.strokeStyle = this.strokeStyle;
      ctx.beginPath();
      for (const p of this.data) {
        if (!p || typeof p.x !== "number" || p.x < minX || p.x > maxX) continue;
        if (p.y == null || !Number.isFinite(p.y)) {
          started = false;
          continue;
        }
        const x = toX(p.x);
        const y = toY(p.y);
        if (!started) {
          ctx.moveTo(x, y);
          started = true;
        } else {
          ctx.lineTo(x, y);
        }
      }
      if (!started) return;

      if (this.fillStyle) {
        // Re-run to build a closed area; keep this simple & cheap.
        ctx.save();
        ctx.globalCompositeOperation = "source-over";
        ctx.fillStyle = this.fillStyle;
        ctx.beginPath();
        started = false;
        let firstX = 0;
        let lastX = 0;
        for (const p of this.data) {
          if (!p || typeof p.x !== "number" || p.x < minX || p.x > maxX) continue;
          if (p.y == null || !Number.isFinite(p.y)) {
            started = false;
            continue;
          }
          const x = toX(p.x);
          const y = toY(p.y);
          if (!started) {
            firstX = x;
            ctx.moveTo(x, h);
            ctx.lineTo(x, y);
            started = true;
          } else {
            ctx.lineTo(x, y);
          }
          lastX = x;
        }
        if (started) {
          ctx.lineTo(lastX, h);
          ctx.lineTo(firstX, h);
          ctx.closePath();
          ctx.fill();
        }
        ctx.restore();
      }

      ctx.stroke();
    },
  };

  return spark;
}

function makeSparks() {
  const stroke = cssVar("--chart-strong", "rgba(250, 250, 250, 0.92)");
  const fill = withAlpha(stroke, 0.08);

  const sparks = {
    elapsed: makeSpark("spark_elapsed", stroke, fill),
    vus: makeSpark("spark_vus", stroke, fill),
    rps: makeSpark("spark_rps", stroke, fill),
    ips: makeSpark("spark_ips", stroke, fill),
    fail: makeSpark("spark_fail", stroke, fill),
    httpP95: makeSpark("spark_http_p95", stroke, fill),

    reset() {
      for (const s of Object.values(this)) {
        if (s && typeof s.reset === "function") s.reset();
      }
    },

    pushPoint(p) {
      const x = p.elapsedSeconds;
      if (typeof x !== "number") return;
      this.elapsed?.push(x, x);
      this.vus?.push(x, p.vus);
      this.rps?.push(x, p.requestsPerSec);
      this.ips?.push(x, p.iterationsPerSec);
      this.fail?.push(x, (p.errorRate ?? 0) * 100);
      this.httpP95?.push(x, p.httpReqDurationP95Ms ?? null);
    },

    applyWindow(maxX, windowSeconds) {
      const minX = Math.max(0, maxX - windowSeconds);
      for (const s of [this.elapsed, this.vus, this.rps, this.ips, this.fail, this.httpP95]) {
        if (!s) continue;
        s.trim(minX);
        s.setWindow(minX, maxX);
      }
    },

    applyFullWindow(maxX) {
      for (const s of [this.elapsed, this.vus, this.rps, this.ips, this.fail, this.httpP95]) {
        if (!s) continue;
        s.setWindow(0, maxX);
      }
    },

    draw() {
      for (const s of [this.elapsed, this.vus, this.rps, this.ips, this.fail, this.httpP95]) {
        s?.draw();
      }
    },
  };

  return sparks;
}

function makeCharts() {
  const gridColor = "rgba(250, 250, 250, 0.06)";
  const tickColor = "rgba(250, 250, 250, 0.55)";
  const titleColor = "rgba(250, 250, 250, 0.55)";
  const cStrong = cssVar("--chart-strong", "rgba(250, 250, 250, 0.92)");
  const cMid = cssVar("--chart-mid", "rgba(250, 250, 250, 0.60)");
  const cSoft = cssVar("--chart-soft", "rgba(250, 250, 250, 0.42)");

  const common = {
    type: "line",
    options: {
      responsive: true,
      maintainAspectRatio: false,
      animation: { duration: LIVE_ANIM_MS, easing: "linear" },
      // Smoothly animate appended points: start the new point from the previous
      // point's y-value (instead of from baseline), avoiding the "rising from 0"
      // artifact while keeping the dashboard feeling live.
      animations: {
        x: { duration: LIVE_ANIM_MS, easing: "linear" },
        y: {
          duration: LIVE_ANIM_MS,
          easing: "linear",
          from(ctx) {
            const i = ctx.dataIndex;
            const ds = ctx.chart.data.datasets?.[ctx.datasetIndex]?.data;
            if (!Array.isArray(ds) || i <= 0) return ctx.parsed?.y;
            const prev = ds[i - 1];
            return prev && typeof prev.y === "number" ? prev.y : ctx.parsed?.y;
          },
        },
      },
      parsing: false,
      normalized: true,
      interaction: { mode: "index", intersect: false },
      scales: {
        x: {
          type: "linear",
          grid: { color: gridColor },
          ticks: { color: tickColor, maxTicksLimit: 7 },
          title: { display: true, text: "elapsed (s)", color: titleColor },
        },
      },
      plugins: {
        legend: { display: false },
        tooltip: {
          backgroundColor: "rgba(9, 9, 11, 0.96)",
          borderColor: "rgba(250, 250, 250, 0.10)",
          borderWidth: 1,
          titleColor: "rgba(250, 250, 250, 0.90)",
          bodyColor: "rgba(250, 250, 250, 0.78)",
          padding: 10,
          displayColors: true,
        },
      },
      elements: {
        point: { radius: 0, hitRadius: 12 },
        // NOTE: non-zero tension will re-compute bezier control points for prior
        // segments when a new point arrives, which looks like the whole line
        // "jumps" every tick. Keep it straight/monotone for stable streaming.
        line: {
          borderWidth: 1.6,
          tension: 0,
          cubicInterpolationMode: "monotone",
        },
      },
    },
  };

  const perf = new Chart(document.getElementById("chart_perf"), {
    ...common,
    data: {
      datasets: [
        {
          label: "Request Rate",
          borderColor: cStrong,
          fill: false,
          yAxisID: "y_rps",
          data: [],
        },
        {
          label: "Request Duration p(95)",
          borderColor: cMid,
          borderDash: [6, 4],
          fill: false,
          yAxisID: "y_ms",
          data: [],
        },
        {
          label: "Request Failed",
          borderColor: cSoft,
          fill: false,
          yAxisID: "y_pct",
          data: [],
        },
      ],
    },
    options: {
      ...common.options,
      plugins: {
        ...common.options.plugins,
        legend: {
          display: true,
          labels: { color: tickColor, boxWidth: 10, boxHeight: 10 },
        },
      },
      scales: {
        ...common.options.scales,
        y_rps: {
          position: "left",
          grid: { color: gridColor },
          ticks: { color: tickColor, callback: (v) => fmtRate(v) },
          title: { display: true, text: "req/s", color: titleColor },
        },
        y_ms: {
          position: "right",
          grid: { drawOnChartArea: false },
          ticks: { color: tickColor, callback: (v) => fmtMs(v) },
          title: { display: true, text: "ms", color: titleColor },
        },
        y_pct: {
          position: "right",
          grid: { drawOnChartArea: false },
          ticks: { color: tickColor, callback: (v) => fmtPct(v) },
          title: { display: true, text: "%", color: titleColor },
        },
      },
    },
  });

  const vus = new Chart(document.getElementById("chart_vus"), {
    ...common,
    data: {
      datasets: [
        {
          label: "vus",
          borderColor: cStrong,
          fill: false,
          yAxisID: "y_vus",
          data: [],
        },
        {
          label: "vus_max",
          borderColor: cSoft,
          borderDash: [6, 4],
          fill: false,
          yAxisID: "y_vus",
          data: [],
        },
      ],
    },
    options: {
      ...common.options,
      plugins: {
        ...common.options.plugins,
        legend: {
          display: true,
          labels: { color: tickColor, boxWidth: 10, boxHeight: 10 },
        },
      },
      scales: {
        ...common.options.scales,
        y_vus: {
          position: "left",
          grid: { color: gridColor },
          ticks: { color: tickColor, callback: (v) => fmtInt(v) },
          title: { display: true, text: "vus", color: titleColor },
        },
      },
    },
  });

  const xfer = new Chart(document.getElementById("chart_xfer"), {
    ...common,
    data: {
      datasets: [
        {
          label: "bytes received/sec",
          borderColor: cStrong,
          fill: false,
          data: [],
        },
        {
          label: "bytes sent/sec",
          borderColor: cSoft,
          borderDash: [6, 4],
          fill: false,
          data: [],
        },
      ],
    },
    options: {
      ...common.options,
      plugins: {
        ...common.options.plugins,
        legend: {
          display: true,
          labels: { color: tickColor, boxWidth: 10, boxHeight: 10 },
        },
      },
      scales: {
        ...common.options.scales,
        y: {
          grid: { color: gridColor },
          ticks: {
            color: tickColor,
            callback: (v) => fmtBytesPerSec(v),
          },
          title: { display: true, text: "transfer", color: titleColor },
        },
      },
    },
  });

  const httpDur = new Chart(document.getElementById("chart_http_dur"), {
    ...common,
    data: {
      datasets: [
        {
          label: "avg",
          borderColor: cStrong,
          fill: false,
          data: [],
        },
        {
          label: "p90",
          borderColor: cMid,
          fill: false,
          data: [],
        },
        {
          label: "p95",
          borderColor: cSoft,
          borderDash: [6, 4],
          fill: false,
          data: [],
        },
        {
          label: "p99",
          borderColor: cSoft,
          borderDash: [2, 3],
          fill: false,
          data: [],
        },
      ],
    },
    options: {
      ...common.options,
      plugins: {
        ...common.options.plugins,
        legend: {
          display: true,
          labels: { color: tickColor, boxWidth: 10, boxHeight: 10 },
        },
      },
      scales: {
        ...common.options.scales,
        y: {
          grid: { color: gridColor },
          ticks: { color: tickColor, callback: (v) => fmtMs(v) },
          title: { display: true, text: "ms", color: titleColor },
        },
      },
    },
  });

  const iterDur = new Chart(document.getElementById("chart_iter_dur"), {
    ...common,
    data: {
      datasets: [
        {
          label: "avg",
          borderColor: cStrong,
          fill: false,
          data: [],
        },
        {
          label: "p90",
          borderColor: cMid,
          fill: false,
          data: [],
        },
        {
          label: "p95",
          borderColor: cSoft,
          borderDash: [6, 4],
          fill: false,
          data: [],
        },
        {
          label: "p99",
          borderColor: cSoft,
          borderDash: [2, 3],
          fill: false,
          data: [],
        },
      ],
    },
    options: {
      ...common.options,
      plugins: {
        ...common.options.plugins,
        legend: {
          display: true,
          labels: { color: tickColor, boxWidth: 10, boxHeight: 10 },
        },
      },
      scales: {
        ...common.options.scales,
        y: {
          grid: { color: gridColor },
          ticks: { color: tickColor, callback: (v) => fmtMs(v) },
          title: { display: true, text: "ms", color: titleColor },
        },
      },
    },
  });

  return { perf, vus, xfer, httpDur, iterDur };
}

function setXWindow(chart, minX, maxX) {
  const x = chart?.options?.scales?.x;
  if (!x) return;
  x.min = minX;
  x.max = maxX;
}

function trimSeries(data, minX) {
  if (!Array.isArray(data) || data.length === 0) return;
  while (data.length > 0 && data[0] && typeof data[0].x === "number" && data[0].x < minX) {
    data.shift();
  }
}

function applyLiveWindow(charts, elapsedSeconds, windowSeconds) {
  const maxX = elapsedSeconds;
  const minX = Math.max(0, maxX - windowSeconds);

  // Keep x-scale stable across updates.
  setXWindow(charts.perf, minX, maxX);
  setXWindow(charts.vus, minX, maxX);
  setXWindow(charts.xfer, minX, maxX);
  setXWindow(charts.httpDur, minX, maxX);
  setXWindow(charts.iterDur, minX, maxX);

  // Drop points outside the window to keep rendering cheap.
  for (const ds of charts.perf.data.datasets) trimSeries(ds.data, minX);
  for (const ds of charts.vus.data.datasets) trimSeries(ds.data, minX);
  for (const ds of charts.xfer.data.datasets) trimSeries(ds.data, minX);
  for (const ds of charts.httpDur.data.datasets) trimSeries(ds.data, minX);
  for (const ds of charts.iterDur.data.datasets) trimSeries(ds.data, minX);
}

function updateCharts(charts, mode) {
  charts.perf.update(mode);
  charts.vus.update(mode);
  charts.xfer.update(mode);
  charts.httpDur.update(mode);
  charts.iterDur.update(mode);
}

function applyPoint(charts, p) {
  charts.perf.data.datasets[0].data.push({ x: p.elapsedSeconds, y: p.requestsPerSec });
  charts.perf.data.datasets[1].data.push({ x: p.elapsedSeconds, y: p.httpReqDurationP95Ms ?? null });
  charts.perf.data.datasets[2].data.push({ x: p.elapsedSeconds, y: (p.errorRate ?? 0) * 100 });

  charts.vus.data.datasets[0].data.push({ x: p.elapsedSeconds, y: p.vus });
  charts.vus.data.datasets[1].data.push({ x: p.elapsedSeconds, y: p.vusMax });

  charts.xfer.data.datasets[0].data.push({ x: p.elapsedSeconds, y: p.bytesReceivedPerSec });
  charts.xfer.data.datasets[1].data.push({ x: p.elapsedSeconds, y: p.bytesSentPerSec });

  charts.httpDur.data.datasets[0].data.push({ x: p.elapsedSeconds, y: p.httpReqDurationAvgMs ?? null });
  charts.httpDur.data.datasets[1].data.push({ x: p.elapsedSeconds, y: p.httpReqDurationP90Ms ?? null });
  charts.httpDur.data.datasets[2].data.push({ x: p.elapsedSeconds, y: p.httpReqDurationP95Ms ?? null });
  charts.httpDur.data.datasets[3].data.push({ x: p.elapsedSeconds, y: p.httpReqDurationP99Ms ?? null });

  charts.iterDur.data.datasets[0].data.push({ x: p.elapsedSeconds, y: p.iterationDurationAvgMs ?? null });
  charts.iterDur.data.datasets[1].data.push({ x: p.elapsedSeconds, y: p.iterationDurationP90Ms ?? null });
  charts.iterDur.data.datasets[2].data.push({ x: p.elapsedSeconds, y: p.iterationDurationP95Ms ?? null });
  charts.iterDur.data.datasets[3].data.push({ x: p.elapsedSeconds, y: p.iterationDurationP99Ms ?? null });
}

function setText(id, text) {
  const el = document.getElementById(id);
  if (!el) return;
  el.textContent = text;
}

function updateTotals(p) {
  setText("stat_elapsed", fmtDur(p.elapsedSeconds));
  setText("stat_vus", fmtInt(p.vus));
  setText("stat_vus_max", fmtInt(p.vusMax));

  setText("stat_rps", fmtPerSec(p.requestsPerSec));
  setText("stat_ips", fmtPerSec(p.iterationsPerSec));

  setText("stat_reqs", fmtInt(p.requestsTotal));
  setText("stat_failed", fmtInt(p.failedRequestsTotal));
  setText("stat_iters", fmtInt(p.iterationsTotal));

  const totalFailRate =
    p.requestsTotal > 0 ? (p.failedRequestsTotal / p.requestsTotal) * 100 : 0;
  setText("stat_fail_rate", fmtPct(totalFailRate));

  setText("tot_checks_failed", fmtInt(p.checksFailedTotal));
  setText("tot_bin", fmtInt(p.bytesReceivedTotal));
  setText("tot_bout", fmtInt(p.bytesSentTotal));

  setText("stat_http_p95", fmtMs(p.httpReqDurationP95Ms));
  setText("stat_http_p99", fmtMs(p.httpReqDurationP99Ms));
  setText("tot_iter_p95", fmtMs(p.iterationDurationP95Ms));
  setText("tot_iter_p99", fmtMs(p.iterationDurationP99Ms));
}

function renderFromSnapshot(snapshotEvt, opts) {
  const snap = snapshotEvt?.snapshot ?? snapshotEvt;
  const points = snap?.points ?? [];
  const charts = makeCharts();
  const sparks = makeSparks();

  const windowSeconds = opts?.windowSeconds ?? null;
  const maxX = points.length > 0 ? points[points.length - 1].elapsedSeconds : 0;
  const minX =
    windowSeconds != null && Number.isFinite(windowSeconds)
      ? Math.max(0, maxX - windowSeconds)
      : null;

  for (const p of points) {
    if (minX != null && p.elapsedSeconds < minX) continue;
    applyPoint(charts, p);
    sparks.pushPoint(p);
  }
  if (minX != null) {
    applyLiveWindow(charts, maxX, windowSeconds);
    sparks.applyWindow(maxX, windowSeconds);
  } else {
    sparks.applyFullWindow(maxX);
  }
  if (points.length > 0) {
    updateTotals(points[points.length - 1]);
  }
  updateCharts(charts, "none");
  sparks.draw();

  return { charts, sparks, pointsCount: points.length };
}

function setStatus(text) {
  const el = document.getElementById("status");
  if (el) el.textContent = text;
}

(function main() {
  const mode = window.__WRKR_DASHBOARD_MODE__;

  if (mode === "offline") {
    const snap = getSnapshotFromDom();
    renderFromSnapshot(snap);
    setStatus("completed");
    return;
  }

  // live
  const charts = makeCharts();
  const sparks = makeSparks();
  let lastPoint = null;
  let lastElapsed = 0;

  const es = new EventSource("/events");
  es.addEventListener("snapshot", (ev) => {
    try {
      const msg = JSON.parse(ev.data);
      // Replace visible data with the last N seconds from snapshot.
      for (const chart of [charts.perf, charts.vus, charts.xfer, charts.httpDur, charts.iterDur]) {
        for (const ds of chart.data.datasets) ds.data = [];
      }
      sparks.reset();

      const snap = msg.snapshot;
      const points = snap?.points || [];
      const maxX = points.length > 0 ? points[points.length - 1].elapsedSeconds : 0;
      const minX = Math.max(0, maxX - LIVE_WINDOW_SECONDS);

      for (const p of points) {
        if (p.elapsedSeconds < minX) continue;
        applyPoint(charts, p);
        sparks.pushPoint(p);
        lastPoint = p;
      }
      if (lastPoint) updateTotals(lastPoint);
      applyLiveWindow(charts, maxX, LIVE_WINDOW_SECONDS);
      sparks.applyWindow(maxX, LIVE_WINDOW_SECONDS);
      updateCharts(charts, "none");
      sparks.draw();
      lastElapsed = maxX;
    } catch {
      // ignore
    }
  });

  es.addEventListener("tick", (ev) => {
    try {
      const msg = JSON.parse(ev.data);
      const p = msg.point;
      if (!p) return;
      applyPoint(charts, p);
      sparks.pushPoint(p);
      lastPoint = p;
      updateTotals(p);
      applyLiveWindow(charts, p.elapsedSeconds, LIVE_WINDOW_SECONDS);
      sparks.applyWindow(p.elapsedSeconds, LIVE_WINDOW_SECONDS);
      updateCharts(charts, "none");
      sparks.draw();
      lastElapsed = p.elapsedSeconds;
    } catch {
      // ignore
    }
  });

  window.addEventListener("resize", () => {
    if (!Number.isFinite(lastElapsed) || lastElapsed <= 0) return;
    sparks.applyWindow(lastElapsed, LIVE_WINDOW_SECONDS);
    sparks.draw();
  });

  es.addEventListener("done", () => {
    setStatus("completed");
    es.close();
  });

  es.onerror = () => {
    // Keep the status minimal; browser will auto-reconnect.
    setStatus("running… (reconnecting)");
  };
})();
