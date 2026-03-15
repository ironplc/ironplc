import uPlot from './uPlot.esm.js';

const editor = document.getElementById("editor");
const startBtn = document.getElementById("start-btn");
const stopBtn = document.getElementById("stop-btn");
const pauseBtn = document.getElementById("pause-btn");
const intervalInput = document.getElementById("interval-input");
const durationDisplay = document.getElementById("duration-display");
const cyclesDisplay = document.getElementById("cycles-display");
const status = document.getElementById("status");
const variablesPanel = document.getElementById("variables-panel");
const diagnosticsPanel = document.getElementById("diagnostics-panel");
const examplesSelect = document.getElementById("examples-select");

// --- Example programs ---

const EXAMPLES = [
  {
    name: "Counter",
    code: `PROGRAM main
  VAR
    count : INT;
    doubled : INT;
  END_VAR

  (* Click Start to begin running. The program
     runs one scan cycle per interval. Variables
     keep their values between cycles, so count
     increases each time. *)
  count := count + 1;
  doubled := count * 2;
END_PROGRAM`,
  },
  {
    name: "Boolean Logic",
    code: `PROGRAM main
  VAR
    sensor_a : BOOL := TRUE;
    sensor_b : BOOL := FALSE;
    and_result : BOOL;
    or_result : BOOL;
    not_result : BOOL;
  END_VAR

  and_result := sensor_a AND sensor_b;
  or_result := sensor_a OR sensor_b;
  not_result := NOT sensor_a;
END_PROGRAM`,
  },
  {
    name: "Arithmetic",
    code: `PROGRAM main
  VAR
    a : INT := 10;
    b : INT := 3;
    sum : INT;
    diff : INT;
    product : INT;
    quotient : INT;
    remainder : INT;
  END_VAR

  sum := a + b;
  diff := a - b;
  product := a * b;
  quotient := a / b;
  remainder := a MOD b;
END_PROGRAM`,
  },
  {
    name: "Comparison",
    code: `PROGRAM main
  VAR
    temperature : INT;
    is_hot : BOOL;
    is_cold : BOOL;
    is_moderate : BOOL;
  END_VAR

  temperature := temperature + 1;

  is_hot := temperature > 30;
  is_cold := temperature < 10;
  is_moderate := NOT is_hot AND NOT is_cold;
END_PROGRAM`,
  },
  {
    name: "Sine Wave",
    code: `PROGRAM main
  VAR
    angle : REAL;
    wave : REAL;
    output : INT;
  END_VAR

  (* Generate a sine wave. In real PLCs this is
     used for motion profiles, test signal
     generation, and vibration compensation. *)
  angle := angle + 0.1;
  wave := SIN(angle);

  (* Scale to 0-100 range for an analog output *)
  output := REAL_TO_INT(wave * 50.0 + 50.0);
END_PROGRAM`,
  },
];

// --- State ---

let stepIntervalId = null;
let renderIntervalId = null;
let isRunning = false;
let isPaused = false;
let cycleCount = 0;
let startTime = 0;
let pausedElapsed = 0;
let lastVariables = null;
let stepInFlight = false;
let previousValues = new Map();
let compilerVersion = "";

// --- URL parameter handling ---

const params = new URLSearchParams(window.location.search);
const isEmbed = params.get("embed") === "true";

// --- Sparkline history ---

const STEP_INTERVAL_MS = 100;
const RENDER_INTERVAL_MS = 500;
const HISTORY_WINDOW_MS = 5000;
let valueHistory = new Map();

// --- uPlot sparkline options ---

const sparkWidth = isEmbed ? 70 : 120;
const sparkHeight = isEmbed ? 18 : 24;

function makeSparkOpts(stepped) {
  return {
    width: sparkWidth,
    height: sparkHeight,
    pxAlign: false,
    cursor: { show: false },
    select: { show: false },
    legend: { show: false },
    scales: { x: { time: false, range: [0, HISTORY_WINDOW_MS / 1000] } },
    axes: [{ show: false }, { show: false }],
    series: [
      {},
      {
        stroke: "#6c8cff",
        width: 1.5 / devicePixelRatio,
        paths: stepped ? uPlot.paths.stepped({ align: 1 }) : undefined,
      },
    ],
  };
}

const sparkOpts = makeSparkOpts(false);
const boolSparkOpts = makeSparkOpts(true);

if (isEmbed) {
  document.body.classList.add("embed");
  intervalInput.disabled = true;
}

// --- Populate examples dropdown ---

for (const example of EXAMPLES) {
  const option = document.createElement("option");
  option.value = example.name;
  option.textContent = example.name;
  examplesSelect.appendChild(option);
}

examplesSelect.addEventListener("change", () => {
  const selected = EXAMPLES.find(e => e.name === examplesSelect.value);
  if (!selected) return;

  if (isRunning) {
    stopExecution();
    postCommand("reset");
    status.textContent = "Ready";
  }

  editor.value = selected.code;

  // Reset the dropdown to show "Examples" label
  examplesSelect.selectedIndex = 0;
});

// Pre-load code from URL parameters
if (params.has("code")) {
  try {
    const decoded = atob(params.get("code"));
    let code = decoded;

    // Scaffold mode: wrap snippet in PROGRAM block
    if (params.get("scaffold") === "true") {
      const trimmed = decoded.trimStart();
      const startsWithPOU =
        /^(PROGRAM|FUNCTION_BLOCK|FUNCTION)\s/i.test(trimmed);
      if (!startsWithPOU) {
        let varBlock = "";
        if (params.has("vars")) {
          const vars = atob(params.get("vars"));
          varBlock = vars
            .split(";")
            .filter((v) => v.trim())
            .map((v) => `    ${v.trim()};`)
            .join("\n");
        }
        const varSection =
          varBlock ? `  VAR\n${varBlock}\n  END_VAR\n` : "";
        code = `PROGRAM main\n${varSection}  ${trimmed.replace(/\n/g, "\n  ")}\nEND_PROGRAM\n`;
      }
    }

    editor.value = code;
  } catch (e) {
    // Ignore invalid base64
  }
}

// --- Web Worker communication ---

const worker = new Worker("worker.js", { type: "module" });
let nextId = 1;
const pending = new Map();

worker.onmessage = (e) => {
  const msg = e.data;

  if (msg.type === "ready") {
    compilerVersion = msg.version || "";
    startBtn.disabled = false;
    status.textContent = "Ready";
    return;
  }

  if (msg.type === "error" && !msg.id) {
    status.textContent = msg.error;
    return;
  }

  const resolve = pending.get(msg.id);
  if (resolve) {
    pending.delete(msg.id);
    resolve(msg);
  }
};

function postCommand(command, params) {
  return new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, resolve);
    worker.postMessage({ id, command, ...params });
  });
}

// --- Tab switching ---

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
    tab.classList.add("active");
    const panel = document.getElementById(`${tab.dataset.tab}-panel`);
    panel.classList.add("active");
  });
});

// --- Transport controls ---

function getIntervalMs() {
  const val = parseInt(intervalInput.value, 10);
  return val > 0 ? val : 500;
}

function startStepLoop() {
  stepIntervalId = setInterval(async () => {
    if (stepInFlight) return;
    stepInFlight = true;

    const before = performance.now();
    const msg = await postCommand("step", { scans: 1 });
    const elapsed = performance.now() - before;
    stepInFlight = false;

    if (!isRunning || isPaused) return;

    if (msg.type === "error") {
      stopExecution();
      status.textContent = msg.error;
      return;
    }

    const result = JSON.parse(msg.json);
    if (!result.ok) {
      stopExecution();
      if (result.diagnostics && result.diagnostics.length > 0) {
        renderDiagnostics(result.diagnostics);
        activateTab("diagnostics");
        status.textContent = `${result.diagnostics.length} error(s)`;
      } else if (result.error) {
        renderVariables(result.variables || []);
        diagnosticsPanel.innerHTML = `<p class="error-message">${escapeHtml(result.error)}</p>`;
        status.textContent = "Runtime error";
        activateTab("diagnostics");
      }
      return;
    }

    // Check for cycle overrun against the step interval
    if (elapsed > STEP_INTERVAL_MS) {
      stopExecution();
      status.textContent = `Cycle overrun: execution took ${elapsed.toFixed(0)}ms but step interval is ${STEP_INTERVAL_MS}ms`;
      diagnosticsPanel.innerHTML = `<p class="error-message">Cycle overrun: program execution took ${elapsed.toFixed(0)}ms but the step interval is ${STEP_INTERVAL_MS}ms. Reduce program complexity or increase the interval.</p>`;
      activateTab("diagnostics");
      return;
    }

    cycleCount = result.total_scans;
    lastVariables = result.variables;
    accumulateHistory(result.variables);
  }, STEP_INTERVAL_MS);
}

function startRenderLoop() {
  updateDisplays();
  renderIntervalId = setInterval(() => {
    updateDisplays();
  }, RENDER_INTERVAL_MS);
}

function updateDisplays() {
  const elapsed = performance.now() - startTime + pausedElapsed;
  durationDisplay.textContent = (elapsed / 1000).toFixed(1) + "s";
  cyclesDisplay.textContent = `${cycleCount} cycles`;

  if (lastVariables) {
    renderVariables(lastVariables);
    activateTab("variables");
  }
}

function stopExecution() {
  clearInterval(stepIntervalId);
  clearInterval(renderIntervalId);
  stepIntervalId = null;
  renderIntervalId = null;
  isRunning = false;
  isPaused = false;
  stepInFlight = false;
  resetTransportButtons();
}

function resetTransportButtons() {
  startBtn.disabled = false;
  startBtn.classList.remove("active");
  stopBtn.disabled = true;
  pauseBtn.disabled = true;
  pauseBtn.classList.remove("active");
  if (!isEmbed) {
    intervalInput.disabled = false;
  }
}

// --- Start ---

startBtn.addEventListener("click", async () => {
  const source = editor.value;
  const intervalMs = getIntervalMs();
  const cycleTimeUs = intervalMs * 1000;

  startBtn.disabled = true;
  stopBtn.disabled = true;
  pauseBtn.disabled = true;
  intervalInput.disabled = true;

  status.textContent = "Compiling\u2026";

  const loadMsg = await postCommand("load_program", { source, cycleTimeUs });

  if (loadMsg.type === "error") {
    status.textContent = loadMsg.error;
    resetTransportButtons();
    return;
  }

  const loadResult = JSON.parse(loadMsg.json);
  if (!loadResult.ok) {
    if (loadResult.diagnostics && loadResult.diagnostics.length > 0) {
      renderDiagnostics(loadResult.diagnostics);
      activateTab("diagnostics");
      status.textContent = `${loadResult.diagnostics.length} error(s)`;
    } else if (loadResult.error) {
      status.textContent = loadResult.error;
    }
    resetTransportButtons();
    return;
  }

  // Reset counters and start
  cycleCount = 0;
  pausedElapsed = 0;
  startTime = performance.now();
  previousValues = new Map();
  valueHistory = new Map();
  lastVariables = null;
  isRunning = true;
  isPaused = false;

  startBtn.disabled = true;
  startBtn.classList.add("active");
  stopBtn.disabled = false;
  pauseBtn.disabled = false;
  intervalInput.disabled = true;

  status.textContent = "Running";

  startStepLoop();
  startRenderLoop();
});

// --- Stop ---

stopBtn.addEventListener("click", async () => {
  const finalVars = lastVariables;
  stopExecution();

  if (finalVars) {
    renderVariables(finalVars);
  }

  await postCommand("reset");
  previousValues = new Map();
  valueHistory = new Map();
  status.textContent = `Stopped after ${cycleCount} cycles`;
});

// --- Pause / Resume ---

pauseBtn.addEventListener("click", () => {
  if (isPaused) {
    // Resume
    isPaused = false;
    pauseBtn.classList.remove("active");
    startTime = performance.now();

    startStepLoop();
    startRenderLoop();

    status.textContent = "Running";
  } else {
    // Pause
    isPaused = true;
    pausedElapsed += performance.now() - startTime;
    pauseBtn.classList.add("active");

    clearInterval(stepIntervalId);
    clearInterval(renderIntervalId);
    stepIntervalId = null;
    renderIntervalId = null;

    // Final render with current state
    updateDisplays();

    status.textContent = `Paused at ${cycleCount} cycles`;
  }
});

// --- Source change handling ---

editor.addEventListener("input", () => {
  if (isRunning) {
    stopExecution();
    postCommand("reset");
    status.textContent = "Source changed \u2014 stopped. Click Start to recompile.";
  }
});

// --- Value parsing for sparklines ---

function parseNumericValue(value, typeName) {
  const t = typeName.toUpperCase();

  if (t === "BOOL") {
    return value === "TRUE" ? 1 : 0;
  }

  if (["SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT"].includes(t)) {
    const n = Number(value);
    return Number.isFinite(n) ? n : null;
  }

  if (t === "REAL" || t === "LREAL") {
    const n = parseFloat(value);
    return Number.isFinite(n) ? n : null;
  }

  if (["BYTE", "WORD", "DWORD", "LWORD"].includes(t)) {
    if (value.startsWith("16#")) {
      const n = parseInt(value.slice(3), 16);
      return Number.isFinite(n) ? n : null;
    }
    return null;
  }

  if (t === "TIME") {
    return parseTimeValue(value);
  }

  return null;
}

function parseTimeValue(value) {
  const match = value.match(/^(-?)T#([\d.]+)(ms|s)$/);
  if (!match) return null;
  const sign = match[1] === "-" ? -1 : 1;
  const num = parseFloat(match[2]);
  const unit = match[3];
  const ms = unit === "s" ? num * 1000 : num;
  return Number.isFinite(ms) ? sign * ms : null;
}

function accumulateHistory(variables) {
  const now = performance.now();
  const cutoff = now - HISTORY_WINDOW_MS;
  for (const v of variables) {
    const numVal = parseNumericValue(v.value, v.type_name);
    if (numVal !== null) {
      let hist = valueHistory.get(v.index);
      if (!hist) {
        hist = [];
        valueHistory.set(v.index, hist);
      }
      hist.push({ t: now, v: numVal });
      // Drop entries older than the window
      while (hist.length > 0 && hist[0].t < cutoff) {
        hist.shift();
      }
    }
  }
}

// --- Display helpers ---

function renderVariables(variables) {
  if (!variables || variables.length === 0) {
    variablesPanel.innerHTML = '<p class="placeholder">No variables.</p>';
    return;
  }

  let html = '<table class="var-table"><thead><tr><th>Variable</th><th>Value</th><th>History</th></tr></thead><tbody>';
  for (const v of variables) {
    const prev = previousValues.get(v.index);
    const changed = prev !== undefined && prev !== v.value;
    html += `<tr${changed ? ' class="changed"' : ''}>`;
    const label = v.name ? `${escapeHtml(v.name)} : ${escapeHtml(v.type_name)}` : `var[${v.index}]`;
    html += `<td>${label}</td><td>${v.value}</td>`;
    html += `<td class="sparkline-cell" data-var-idx="${v.index}"></td>`;
    html += '</tr>';
  }
  html += "</tbody></table>";
  variablesPanel.innerHTML = html;

  // Create uPlot sparklines in the empty cells
  const now = performance.now();
  const windowStart = now - HISTORY_WINDOW_MS;
  for (const v of variables) {
    const hist = valueHistory.get(v.index);
    if (hist && hist.length >= 2) {
      const cell = variablesPanel.querySelector(`[data-var-idx="${v.index}"]`);
      if (cell) {
        const xs = hist.map(e => (e.t - windowStart) / 1000);
        const ys = hist.map(e => e.v);
        const opts = v.type_name.toUpperCase() === "BOOL" ? boolSparkOpts : sparkOpts;
        new uPlot(opts, [xs, ys], cell);
      }
    }
  }

  previousValues = new Map(variables.map(v => [v.index, v.value]));
}

function renderDiagnostics(diagnostics) {
  let html = "";
  for (const d of diagnostics) {
    html += '<div class="diagnostic-item">';
    const code = escapeHtml(d.code);
    if (/^P\d{4}$/.test(d.code)) {
      const url = `https://www.ironplc.com/reference/compiler/problems/${d.code}.html?version=${encodeURIComponent(compilerVersion)}`;
      html += `<a class="diagnostic-code" href="${url}" target="_blank" rel="noopener">${code}</a>`;
    } else {
      html += `<span class="diagnostic-code">${code}</span>`;
    }
    let message = escapeHtml(d.message);
    if (d.label) {
      message += `: ${escapeHtml(d.label)}`;
    }
    html += `<span class="diagnostic-message">${message}</span>`;
    if (d.start > 0 || d.end > 0) {
      html += `<span class="diagnostic-location">offset ${d.start}\u2013${d.end}</span>`;
    }
    html += "</div>";
  }
  diagnosticsPanel.innerHTML = html;
}

function activateTab(tabName) {
  document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
  document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
  const tab = document.querySelector(`.tab[data-tab="${tabName}"]`);
  const panel = document.getElementById(`${tabName}-panel`);
  if (tab) tab.classList.add("active");
  if (panel) panel.classList.add("active");
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}
