import uPlot from "./uPlot.esm.js";
import type {
  Diagnostic,
  DialectOption,
  RunError,
  RunResult,
  Variable,
  WorkerRequest,
  WorkerResponse,
} from "./types/messages.js";

const editor = document.getElementById("editor") as HTMLTextAreaElement;
const gutter = document.getElementById("editor-gutter") as HTMLElement;
const startBtn = document.getElementById("start-btn") as HTMLButtonElement;
const stopBtn = document.getElementById("stop-btn") as HTMLButtonElement;
const pauseBtn = document.getElementById("pause-btn") as HTMLButtonElement;
const intervalInput = document.getElementById("interval-input") as HTMLInputElement;
const durationDisplay = document.getElementById("duration-display") as HTMLElement;
const cyclesDisplay = document.getElementById("cycles-display") as HTMLElement;
const statusEl = document.getElementById("status") as HTMLElement;
const variablesPanel = document.getElementById("variables-panel") as HTMLElement;
const diagnosticsPanel = document.getElementById("diagnostics-panel") as HTMLElement;
const examplesSelect = document.getElementById("examples-select") as HTMLSelectElement;
const dialectSelect = document.getElementById("dialect-select") as HTMLSelectElement;
const dialectBadge = document.getElementById("dialect-badge") as HTMLElement;

// --- Example programs ---

interface Example {
  name: string;
  code: string;
}

const EXAMPLES: Example[] = [
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

let stepIntervalId: number | null = null;
let renderIntervalId: number | null = null;
let isRunning = false;
let isPaused = false;
let cycleCount = 0;
let startTime = 0;
let pausedElapsed = 0;
let lastVariables: Variable[] | null = null;
let stepInFlight = false;
let previousValues: Map<number, string> = new Map();
let compilerVersion = "";
let currentIntervalMs = 500;
// The exact source last handed to the compiler. Captured on Start so a report
// reflects what actually produced the diagnostics, even if the user keeps
// typing afterwards.
let lastCompiledSource = "";

// The "P9xxx" codes are compiler errors (unimplemented capabilities and
// internal errors) that we can only fix once we can see the program that
// triggered them, so every P9 code gets the "Submit Code" affordance.
function isReportable(code: string): boolean {
  return /^P9\d{3}$/.test(code);
}

// Cap the source we transmit so a pathological paste can't bloat an event.
const MAX_REPORT_SOURCE_CHARS = 50000;
// Keep the prefilled GitHub issue URL under a length browsers reliably accept.
const MAX_GITHUB_URL_CHARS = 7000;
const GITHUB_NEW_ISSUE_URL = "https://github.com/ironplc/ironplc/issues/new";

// --- URL parameter handling ---

const params = new URLSearchParams(window.location.search);
const isEmbed = params.get("embed") === "true";

// --- Analytics (Clicky pageviews + PostHog events) ---

function trackPageview(path: string, title: string): void {
  if (typeof clicky !== "undefined" && clicky && clicky.log) {
    clicky.log(path, title);
  }
}

function ph(): PostHog | null {
  return typeof posthog !== "undefined" && posthog ? posthog : null;
}

function capture(event: string, props?: Record<string, unknown>): void {
  const p = ph();
  if (p && typeof p.capture === "function") {
    p.capture(event, props || {});
  }
}

function registerSuper(props: Record<string, unknown>): void {
  const p = ph();
  if (p && typeof p.register === "function") {
    p.register(props);
  }
}

let programModifiedRegistered = false;

interface ProgramOrigin {
  program_origin: string;
  example_name?: string | null;
  host_page?: string | null;
}

function setProgramOrigin({ program_origin, example_name = null, host_page = null }: ProgramOrigin): void {
  programModifiedRegistered = false;
  registerSuper({
    program_origin,
    example_name,
    host_page,
    program_modified: false,
  });
}

function markModified(): void {
  if (programModifiedRegistered) return;
  programModifiedRegistered = true;
  registerSuper({ program_modified: true });
}

// A runtime failure (a VM trap, or an infrastructure error like a decode
// failure) carries the same message/code shape as a compiler diagnostic, so
// present it as one. Runtime errors have no source location, so the line/column
// fields are 0 — renderDiagnostics omits the location line when they are.
function runErrorToDiagnostic(error: RunError): Diagnostic {
  return {
    code: error.code ?? "",
    message: error.message,
    start_line: 0,
    start_column: 0,
  };
}

function extractErrorCodes(diagnostics: Diagnostic[] | undefined): string[] {
  if (!diagnostics) return [];
  const seen = new Set<string>();
  const codes: string[] = [];
  for (const d of diagnostics) {
    if (d.code && !seen.has(d.code)) {
      seen.add(d.code);
      codes.push(d.code);
    }
  }
  return codes;
}

// The compiler `file#Lline` locations of reportable (P9xxx) diagnostics. This
// is the compiler's own source location — never the user's program — so it is
// safe to attach to the automatic compile_finished event. It lets us rank the
// most common unimplemented sites without collecting any program.
function extractErrorLocations(diagnostics: Diagnostic[] | undefined): string[] {
  if (!diagnostics) return [];
  const seen = new Set<string>();
  const locations: string[] = [];
  for (const d of diagnostics) {
    if (!isReportable(d.code) || !d.compiler_file) continue;
    const loc = d.compiler_line
      ? `${d.compiler_file}#L${d.compiler_line}`
      : d.compiler_file;
    if (!seen.has(loc)) {
      seen.add(loc);
      locations.push(loc);
    }
  }
  return locations;
}

type StopReason = "user" | "error" | "reload";

function captureRunStopped(reason: StopReason, errorCodes?: string[]): void {
  const durationMs = startTime > 0 ? (performance.now() - startTime + pausedElapsed) : 0;
  const props: Record<string, unknown> = {
    reason,
    duration_ms: durationMs,
    cycle_count: cycleCount,
  };
  if (reason === "error") {
    props.error_codes = errorCodes || [];
  }
  capture("run_stopped", props);
}

function initAnalytics(): void {
  const sourceParam = params.get("source");
  const hostParam = params.get("host");
  let program_origin: string;
  let host_page: string | null = null;
  if (sourceParam === "ironplc-docs" && hostParam) {
    program_origin = "docs";
    host_page = hostParam;
  } else if (params.has("code")) {
    program_origin = "url_shared";
  } else {
    program_origin = "user_defined";
  }
  const allowsSorted = [...allowsParam].sort().join(",");
  registerSuper({
    embed: isEmbed,
    dialect: getDialect(),
    allows: allowsSorted,
    program_origin,
    example_name: null,
    host_page,
    program_modified: false,
  });
  capture("playground_loaded");
}

// --- Sparkline history ---

const RENDER_INTERVAL_MS = 500;
const HISTORY_WINDOW_MS = 5000;

interface HistoryEntry {
  t: number;
  v: number;
}

let valueHistory: Map<number, HistoryEntry[]> = new Map();

// --- uPlot sparkline options ---

const sparkWidth = isEmbed ? 70 : 120;
const sparkHeight = isEmbed ? 18 : 24;

function makeSparkOpts(stepped: boolean) {
  return {
    width: sparkWidth,
    height: sparkHeight,
    pxAlign: false,
    cursor: { show: false },
    select: { show: false },
    legend: { show: false },
    scales: { x: { time: false, range: [0, HISTORY_WINDOW_MS / 1000] as [number, number] } },
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

// `allows` is a comma-separated list of feature flag short names — the part
// after `--allow-` in the CLI (e.g. "sizeof,c-style-comments").
const allowsParam = (params.get("allows") || "")
  .split(",")
  .map((s) => s.trim())
  .filter((s) => s.length > 0);

// Populate the dialect picker from the compiler-provided list (via the WASM
// `dialects()` export), then apply the URL dialect parameter and dialect badge.
// Called once the worker reports the WASM module is ready.
function initDialects(options: DialectOption[]): void {
  dialectSelect.replaceChildren();
  for (const d of options) {
    const opt = document.createElement("option");
    opt.value = d.value;
    opt.textContent = d.label;
    opt.selected = d.is_default;
    dialectSelect.appendChild(opt);
  }

  // A URL dialect parameter (used by embed/Sphinx directives) overrides the
  // default. The value is a canonical dialect name (Dialect::cli_name).
  const dialectParam = params.get("dialect");
  if (dialectParam && options.some((d) => d.value === dialectParam)) {
    dialectSelect.value = dialectParam;
    dialectBadge.textContent =
      dialectSelect.options[dialectSelect.selectedIndex]?.textContent ??
      dialectParam;
    dialectBadge.classList.add("visible");
  }

  // When feature flags are set, override the badge to "Custom" with a hover
  // listing the flags on top of the selected dialect.
  if (allowsParam.length > 0) {
    dialectBadge.textContent = "Custom";
    const flagList = allowsParam.map((s) => `--allow-${s}`).join(", ");
    const dialectLabel =
      dialectSelect.options[dialectSelect.selectedIndex]?.textContent ??
      "default";
    dialectBadge.title = `${dialectLabel} + ${flagList}`;
    dialectBadge.classList.add("visible");
  }

  registerSuper({ dialect: getDialect() });
}

function getDialect(): string {
  return dialectSelect.value;
}

function getAllows(): string {
  return allowsParam.join(",");
}

// Stop execution when dialect changes (same as source change)
dialectSelect.addEventListener("change", () => {
  registerSuper({ dialect: getDialect() });
  if (isRunning) {
    captureRunStopped("user");
    stopExecution();
    postCommand({ command: "reset" });
    statusEl.textContent = "Dialect changed — stopped. Click Start to recompile.";
  }
});

// --- Populate examples dropdown ---

for (const example of EXAMPLES) {
  const option = document.createElement("option");
  option.value = example.name;
  option.textContent = example.name;
  examplesSelect.appendChild(option);
}

examplesSelect.addEventListener("change", () => {
  const selected = EXAMPLES.find((e) => e.name === examplesSelect.value);
  if (!selected) return;

  if (isRunning) {
    captureRunStopped("user");
    stopExecution();
    postCommand({ command: "reset" });
    statusEl.textContent = "Ready";
  }

  editor.value = selected.code;
  renderLineNumbers();
  trackPageview("/playground/example/" + selected.name, selected.name);
  setProgramOrigin({ program_origin: "example", example_name: selected.name });
  capture("example_loaded");

  // Reset the dropdown to show "Examples" label
  examplesSelect.selectedIndex = 0;
});

// --- Line number gutter ---

function renderLineNumbers(): void {
  const lineCount = Math.max(1, editor.value.split("\n").length);
  let text = "1";
  for (let i = 2; i <= lineCount; i++) {
    text += "\n" + i;
  }
  gutter.textContent = text;
  gutter.style.minWidth = `${Math.max(2, String(lineCount).length) + 1}ch`;
}

editor.addEventListener("scroll", () => {
  gutter.scrollTop = editor.scrollTop;
});

// Pre-load code from URL parameters
if (params.has("code")) {
  try {
    const codeParam = params.get("code") || "";
    const decoded = atob(codeParam);
    let code = decoded;

    // Scaffold mode: wrap snippet in PROGRAM block
    if (params.get("scaffold") === "true") {
      const trimmed = decoded.trimStart();
      const startsWithPOU =
        /^(PROGRAM|FUNCTION_BLOCK|FUNCTION)\s/i.test(trimmed);
      if (!startsWithPOU) {
        let varBlock = "";
        if (params.has("vars")) {
          const vars = atob(params.get("vars") || "");
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
  } catch {
    // Ignore invalid base64
  }
}

renderLineNumbers();

// --- Web Worker communication ---

const worker = new Worker("worker.js", { type: "module" });
let nextId = 1;
const pending = new Map<number, (msg: WorkerResponse) => void>();

worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
  const msg = e.data;

  if (msg.type === "ready") {
    compilerVersion = msg.version || "";
    initDialects(msg.dialects);
    initAnalytics();
    startBtn.disabled = false;
    statusEl.textContent = "Ready";
    return;
  }

  if (msg.type === "error" && !msg.id) {
    statusEl.textContent = msg.error;
    return;
  }

  if (msg.id !== undefined) {
    const resolve = pending.get(msg.id);
    if (resolve) {
      pending.delete(msg.id);
      resolve(msg);
    }
  }
};

type DistributiveOmit<T, K extends keyof T> = T extends unknown ? Omit<T, K> : never;
type RequestPayload = DistributiveOmit<WorkerRequest, "id">;

function postCommand(payload: RequestPayload): Promise<WorkerResponse> {
  return new Promise((resolve) => {
    const id = nextId++;
    pending.set(id, resolve);
    worker.postMessage({ id, ...payload } as WorkerRequest);
  });
}

// --- Tab switching ---

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
    tab.classList.add("active");
    const tabName = (tab as HTMLElement).dataset.tab;
    const panel = document.getElementById(`${tabName}-panel`);
    if (panel) panel.classList.add("active");
  });
});

// --- Transport controls ---

function getIntervalMs(): number {
  const val = parseInt(intervalInput.value, 10);
  return val > 0 ? val : 500;
}

function startStepLoop(): void {
  const intervalMs = currentIntervalMs;
  stepIntervalId = window.setInterval(async () => {
    if (stepInFlight) return;
    stepInFlight = true;

    const before = performance.now();
    const msg = await postCommand({ command: "step", scans: 1 });
    const elapsed = performance.now() - before;
    stepInFlight = false;

    if (!isRunning || isPaused) return;

    if (msg.type === "error") {
      captureRunStopped("error", []);
      stopExecution();
      statusEl.textContent = msg.error;
      return;
    }

    if (msg.type !== "result") return;

    const result = JSON.parse(msg.json) as RunResult;
    if (!result.ok) {
      // Compiler diagnostics and a runtime fault share one representation, so
      // fold a runtime error into the diagnostics list and render both alike.
      const compileDiagnostics = result.diagnostics || [];
      const isRuntimeError = compileDiagnostics.length === 0 && !!result.error;
      const diagnostics = result.error
        ? [...compileDiagnostics, runErrorToDiagnostic(result.error)]
        : compileDiagnostics;
      captureRunStopped("error", extractErrorCodes(diagnostics));
      stopExecution();
      if (isRuntimeError) {
        // A trap can leave meaningful variable state; show it alongside.
        renderVariables(result.variables || []);
      }
      renderDiagnostics(diagnostics);
      activateTab("diagnostics");
      statusEl.textContent = isRuntimeError
        ? "Runtime error"
        : `${diagnostics.length} error(s)`;
      return;
    }

    // Check for cycle overrun against the configured step interval
    if (elapsed > intervalMs) {
      captureRunStopped("error", ["CYCLE_OVERRUN"]);
      stopExecution();
      statusEl.textContent = `Cycle overrun: execution took ${elapsed.toFixed(0)}ms but step interval is ${intervalMs}ms`;
      diagnosticsPanel.innerHTML = `<p class="error-message">Cycle overrun: program execution took ${elapsed.toFixed(0)}ms but the step interval is ${intervalMs}ms. Reduce program complexity or increase the interval.</p>`;
      activateTab("diagnostics");
      return;
    }

    cycleCount = result.total_scans;
    lastVariables = result.variables;
    accumulateHistory(result.variables);
  }, intervalMs);
}

function startRenderLoop(): void {
  updateDisplays();
  renderIntervalId = window.setInterval(() => {
    updateDisplays();
  }, RENDER_INTERVAL_MS);
}

function updateDisplays(): void {
  const elapsed = performance.now() - startTime + pausedElapsed;
  durationDisplay.textContent = (elapsed / 1000).toFixed(1) + "s";
  cyclesDisplay.textContent = `${cycleCount} cycles`;

  if (lastVariables) {
    renderVariables(lastVariables);
    activateTab("variables");
  }
}

function stopExecution(): void {
  if (stepIntervalId !== null) clearInterval(stepIntervalId);
  if (renderIntervalId !== null) clearInterval(renderIntervalId);
  stepIntervalId = null;
  renderIntervalId = null;
  isRunning = false;
  isPaused = false;
  stepInFlight = false;
  resetTransportButtons();
}

function resetTransportButtons(): void {
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
  lastCompiledSource = source;
  const intervalMs = getIntervalMs();
  const cycleTimeUs = intervalMs * 1000;
  const programLines = source.split("\n").length;

  startBtn.disabled = true;
  stopBtn.disabled = true;
  pauseBtn.disabled = true;
  intervalInput.disabled = true;

  statusEl.textContent = "Compiling…";

  const dialect = getDialect();
  const allows = getAllows();
  const compileStart = performance.now();
  capture("compile_attempted", { trigger: "manual" });
  const loadMsg = await postCommand({ command: "load_program", source, cycleTimeUs, dialect, allows });
  const compileDurationMs = performance.now() - compileStart;

  if (loadMsg.type === "error") {
    capture("compile_finished", {
      success: false,
      error_codes: [],
      error_count: 0,
      program_lines: programLines,
      duration_ms: compileDurationMs,
    });
    statusEl.textContent = loadMsg.error;
    resetTransportButtons();
    return;
  }

  if (loadMsg.type !== "result") {
    resetTransportButtons();
    return;
  }

  const loadResult = JSON.parse(loadMsg.json) as RunResult;
  if (!loadResult.ok) {
    // Fold a load-time runtime error (e.g. an init-time VM trap) into the
    // diagnostics list so it renders through the same path as compiler errors.
    const diagnostics = loadResult.error
      ? [...(loadResult.diagnostics || []), runErrorToDiagnostic(loadResult.error)]
      : loadResult.diagnostics || [];
    if (diagnostics.length > 0) {
      renderDiagnostics(diagnostics);
      activateTab("diagnostics");
      statusEl.textContent = `${diagnostics.length} error(s)`;
    }
    capture("compile_finished", {
      success: false,
      error_codes: extractErrorCodes(diagnostics),
      error_count: diagnostics.length,
      // Compiler file/line of any P9xxx diagnostics — no program source.
      error_locations: extractErrorLocations(diagnostics),
      program_lines: programLines,
      duration_ms: compileDurationMs,
    });
    resetTransportButtons();
    return;
  }

  capture("compile_finished", {
    success: true,
    error_codes: [],
    error_count: 0,
    program_lines: programLines,
    duration_ms: compileDurationMs,
  });

  // Reset counters and start
  cycleCount = 0;
  pausedElapsed = 0;
  startTime = performance.now();
  previousValues = new Map();
  valueHistory = new Map();
  lastVariables = null;
  isRunning = true;
  isPaused = false;
  currentIntervalMs = intervalMs;

  startBtn.disabled = true;
  startBtn.classList.add("active");
  stopBtn.disabled = false;
  pauseBtn.disabled = false;
  intervalInput.disabled = true;

  statusEl.textContent = "Running";
  capture("run_started", { cycle_interval_ms: intervalMs });

  startStepLoop();
  startRenderLoop();
});

// --- Stop ---

stopBtn.addEventListener("click", async () => {
  const finalVars = lastVariables;
  captureRunStopped("user");
  stopExecution();

  if (finalVars) {
    renderVariables(finalVars);
  }

  await postCommand({ command: "reset" });
  previousValues = new Map();
  valueHistory = new Map();
  statusEl.textContent = `Stopped after ${cycleCount} cycles`;
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

    statusEl.textContent = "Running";
  } else {
    // Pause
    isPaused = true;
    pausedElapsed += performance.now() - startTime;
    pauseBtn.classList.add("active");

    if (stepIntervalId !== null) clearInterval(stepIntervalId);
    if (renderIntervalId !== null) clearInterval(renderIntervalId);
    stepIntervalId = null;
    renderIntervalId = null;

    // Final render with current state
    updateDisplays();

    statusEl.textContent = `Paused at ${cycleCount} cycles`;
  }
});

// --- Source change handling ---

editor.addEventListener("input", () => {
  renderLineNumbers();
  markModified();
  if (isRunning) {
    captureRunStopped("user");
    stopExecution();
    postCommand({ command: "reset" });
    statusEl.textContent = "Source changed — stopped. Click Start to recompile.";
  }
});

window.addEventListener("pagehide", () => {
  if (isRunning) {
    captureRunStopped("reload");
  }
});

// --- Value parsing for sparklines ---

function parseNumericValue(value: string, typeName: string): number | null {
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

  if (t === "TIME" || t === "LTIME") {
    return parseTimeValue(value);
  }

  return null;
}

function parseTimeValue(value: string): number | null {
  const match = value.match(/^(-?)T#([\d.]+)(ms|s)$/);
  if (!match) return null;
  const sign = match[1] === "-" ? -1 : 1;
  const num = parseFloat(match[2]);
  const unit = match[3];
  const ms = unit === "s" ? num * 1000 : num;
  return Number.isFinite(ms) ? sign * ms : null;
}

function accumulateHistory(variables: Variable[]): void {
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

function renderVariables(variables: Variable[]): void {
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
    const valueClass = v.valid === false ? ' class="value-invalid"' : '';
    html += `<td>${label}</td><td${valueClass}>${escapeHtml(v.value)}</td>`;
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
      const cell = variablesPanel.querySelector(`[data-var-idx="${v.index}"]`) as HTMLElement | null;
      if (cell) {
        const xs = hist.map((e) => (e.t - windowStart) / 1000);
        const ys = hist.map((e) => e.v);
        const opts = v.type_name.toUpperCase() === "BOOL" ? boolSparkOpts : sparkOpts;
        new uPlot(opts, [xs, ys], cell);
      }
    }
  }

  previousValues = new Map(variables.map((v) => [v.index, v.value]));
}

function renderDiagnostics(diagnostics: Diagnostic[]): void {
  let html = "";
  for (const d of diagnostics) {
    html += '<div class="diagnostic-item">';
    // Infrastructure errors (e.g. a decode failure) carry no code; skip the
    // code chip rather than render an empty one.
    if (d.code) {
      const code = escapeHtml(d.code);
      if (/^P\d{4}$/.test(d.code)) {
        const url = `https://www.ironplc.com/reference/compiler/problems/${d.code}.html?version=${encodeURIComponent(compilerVersion)}`;
        html += `<a class="diagnostic-code" href="${url}" target="_blank" rel="noopener">${code}</a>`;
      } else {
        html += `<span class="diagnostic-code">${code}</span>`;
      }
    }
    let message = escapeHtml(d.message);
    if (d.label) {
      message += `: ${escapeHtml(d.label)}`;
    }
    html += `<span class="diagnostic-message">${message}</span>`;
    if (d.start_line > 0 && d.start_column > 0) {
      html += `<span class="diagnostic-location">line ${d.start_line}, column ${d.start_column}</span>`;
    }
    for (const note of d.help ?? []) {
      html += `<span class="diagnostic-help">${escapeHtml(note)}</span>`;
    }
    html += "</div>";
  }

  const reportable = diagnostics.filter((d) => isReportable(d.code));
  if (reportable.length > 0) {
    html += reportPanelHtml(reportable);
  }

  diagnosticsPanel.innerHTML = html;

  if (reportable.length > 0) {
    wireReportPanel(reportable);
  }
}

// --- P9xxx "Submit Code" reporting ---
//
// The P9 codes are compiler errors (unimplemented capabilities and internal
// errors). We can only prioritize and fix them once we can see the program that
// triggered them, so we invite the user to send it. Two things are
// non-negotiable in the UX:
//   1. The button says exactly what happens: "Submit Code".
//   2. A consent line makes clear the program is shared and MAY BECOME PUBLIC
//      (this holds for the PostHog path and the GitHub path alike).
// Source is only ever transmitted on the explicit click below — never
// automatically. (The compiler file/line IS reported automatically via
// compile_finished, but that is the compiler's location, not the program.)

const REPORT_CONSENT_HTML =
  "Submitting sends the program in the editor to the IronPLC team so we can fix " +
  "it. <strong>Your code may be published publicly</strong> — for example in a " +
  "GitHub issue. Don’t submit anything confidential.";

function reportPanelHtml(reportable: Diagnostic[]): string {
  const codes = extractErrorCodes(reportable).join(", ");
  return (
    '<div class="report-panel" data-testid="report-panel">' +
    `<p class="report-title">${escapeHtml(codes)}: the compiler can’t handle this yet. Send us the program so we can fix it.</p>` +
    `<p class="report-consent">${REPORT_CONSENT_HTML}</p>` +
    '<div class="report-actions">' +
    '<button type="button" class="submit-code-btn" data-testid="submit-code-btn">Submit Code</button>' +
    `<a class="report-github-link" data-testid="report-github-link" target="_blank" rel="noopener" href="${escapeHtml(buildGithubIssueUrl(reportable))}">or open a GitHub issue</a>` +
    "</div>" +
    "</div>"
  );
}

function wireReportPanel(reportable: Diagnostic[]): void {
  const btn = diagnosticsPanel.querySelector(
    ".submit-code-btn",
  ) as HTMLButtonElement | null;
  if (!btn) return;
  btn.addEventListener("click", () => {
    submitCodeReport(reportable);
    const panel = diagnosticsPanel.querySelector(".report-panel");
    if (panel) {
      panel.innerHTML =
        '<p class="report-confirmation" data-testid="report-confirmation">' +
        "✓ Thank you — your code was submitted. We’ll use it to add support." +
        "</p>";
    }
  });
}

function submitCodeReport(reportable: Diagnostic[]): void {
  const source = lastCompiledSource;
  const truncated = source.length > MAX_REPORT_SOURCE_CHARS;
  capture("todo_report_submitted", {
    error_codes: extractErrorCodes(reportable),
    error_count: reportable.length,
    program: truncated ? source.slice(0, MAX_REPORT_SOURCE_CHARS) : source,
    program_chars: source.length,
    program_lines: source.split("\n").length,
    program_truncated: truncated,
    // Structured compiler `file#Lline` of each reportable site.
    error_locations: extractErrorLocations(reportable),
    diagnostic_labels: reportable.map((d) => d.label || ""),
    dialect: getDialect(),
    allows: getAllows(),
    compiler_version: compilerVersion,
  });
}

// Build a prefilled "new issue" URL. We include the source inline when it fits
// under the URL length limit; otherwise we ask the user to attach it. The
// consent line above the button already covers that this becomes public.
function buildGithubIssueUrl(reportable: Diagnostic[]): string {
  const source = lastCompiledSource;
  const codes = extractErrorCodes(reportable);
  const primaryCode = codes[0] || "P9999";
  const locations = extractErrorLocations(reportable);
  const locationList = locations.map((l) => `- ${l}`).join("\n");
  const header =
    `**What happened**\nThe playground reported ${codes.join(", ")} ` +
    "(a compiler error) for this program.\n\n" +
    (locationList ? `**Compiler locations**\n${locationList}\n\n` : "") +
    `**Compiler version:** ${compilerVersion || "unknown"}\n` +
    `**Dialect:** ${getDialect() || "default"}\n` +
    (getAllows() ? `**Allows:** ${getAllows()}\n` : "");

  const withProgram =
    header + `\n**Program**\n\`\`\`iecst\n${source}\n\`\`\`\n`;
  const withoutProgram =
    header +
    "\n**Program**\nThe program is too large to prefill here — please " +
    "attach the source file to this issue.\n";

  const base = `${GITHUB_NEW_ISSUE_URL}?labels=${encodeURIComponent(primaryCode)}&title=${encodeURIComponent(`${primaryCode} - Compiler problem report`)}`;
  const candidate = `${base}&body=${encodeURIComponent(withProgram)}`;
  if (candidate.length <= MAX_GITHUB_URL_CHARS) {
    return candidate;
  }
  return `${base}&body=${encodeURIComponent(withoutProgram)}`;
}

function activateTab(tabName: string): void {
  document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
  document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
  const tab = document.querySelector(`.tab[data-tab="${tabName}"]`);
  const panel = document.getElementById(`${tabName}-panel`);
  if (tab) tab.classList.add("active");
  if (panel) panel.classList.add("active");
}

function escapeHtml(str: string): string {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}
