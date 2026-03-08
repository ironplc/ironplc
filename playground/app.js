const editor = document.getElementById("editor");
const runBtn = document.getElementById("run-btn");
const stepBtn = document.getElementById("step-btn");
const resetBtn = document.getElementById("reset-btn");
const scansInput = document.getElementById("scans-input");
const fileInput = document.getElementById("file-input");
const status = document.getElementById("status");
const variablesPanel = document.getElementById("variables-panel");
const diagnosticsPanel = document.getElementById("diagnostics-panel");
const dropOverlay = document.getElementById("drop-overlay");

let sourceChanged = true;
let previousValues = new Map();
let compilerVersion = "";

// --- URL parameter handling ---

const params = new URLSearchParams(window.location.search);
const isEmbed = params.get("embed") === "true";

if (isEmbed) {
  document.body.classList.add("embed");
}

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
    runBtn.disabled = false;
    stepBtn.disabled = false;
    resetBtn.disabled = false;
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

// --- Compile & Run from editor ---

runBtn.addEventListener("click", async () => {
  const source = editor.value;
  const scans = parseInt(scansInput.value, 10) || 1;

  status.textContent = "Compiling and running\u2026";
  runBtn.textContent = "\u25A0 Stop";
  runBtn.disabled = true;

  const msg = await postCommand("run_source", { source, scans });
  runBtn.textContent = "\u25B6 Run";
  runBtn.disabled = false;

  if (msg.type === "error") {
    status.textContent = msg.error;
    return;
  }

  const result = JSON.parse(msg.json);
  displayResult(result);
});

// --- Source change tracking ---

editor.addEventListener("input", () => {
  sourceChanged = true;
});

// --- Step through execution ---

stepBtn.addEventListener("click", async () => {
  const scans = parseInt(scansInput.value, 10) || 1;
  stepBtn.textContent = "\u25A0 Stepping\u2026";
  stepBtn.disabled = true;
  resetBtn.disabled = true;

  if (sourceChanged) {
    status.textContent = "Compiling\u2026";
    const loadMsg = await postCommand("load_program", { source: editor.value });
    if (loadMsg.type === "error") {
      status.textContent = loadMsg.error;
      stepBtn.textContent = "\u25B7 Step";
      stepBtn.disabled = false;
      resetBtn.disabled = false;
      return;
    }
    const loadResult = JSON.parse(loadMsg.json);
    if (!loadResult.ok) {
      displayStepResult(loadResult);
      stepBtn.textContent = "\u25B7 Step";
      stepBtn.disabled = false;
      resetBtn.disabled = false;
      return;
    }
    sourceChanged = false;
  }

  status.textContent = "Stepping\u2026";
  const msg = await postCommand("step", { scans });
  stepBtn.textContent = "\u25B7 Step";
  stepBtn.disabled = false;
  resetBtn.disabled = false;

  if (msg.type === "error") {
    status.textContent = msg.error;
    return;
  }

  const result = JSON.parse(msg.json);
  displayStepResult(result);
});

// --- Reset session ---

resetBtn.addEventListener("click", async () => {
  resetBtn.disabled = true;
  stepBtn.disabled = true;

  await postCommand("reset");

  sourceChanged = true;
  previousValues = new Map();
  variablesPanel.innerHTML = '<p class="placeholder">Run a program to see variable values.</p>';
  diagnosticsPanel.innerHTML = '<p class="placeholder">No diagnostics.</p>';
  status.textContent = "Ready";
  stepBtn.disabled = false;
  resetBtn.disabled = false;
});

// --- Load .iplc file ---

fileInput.addEventListener("change", (e) => {
  const file = e.target.files[0];
  if (!file) return;
  loadIplcFile(file);
});

async function loadIplcFile(file) {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const base64 = uint8ArrayToBase64(bytes);
  const scans = parseInt(scansInput.value, 10) || 1;

  status.textContent = `Running ${file.name}\u2026`;
  runBtn.disabled = true;

  const msg = await postCommand("run", { bytecodeBase64: base64, scans });
  runBtn.disabled = false;

  if (msg.type === "error") {
    status.textContent = `${file.name}: ${msg.error}`;
    return;
  }

  const result = JSON.parse(msg.json);
  displayRunResult(result, file.name);
}

// --- Drag and drop ---

let dragCounter = 0;
document.addEventListener("dragenter", (e) => {
  e.preventDefault();
  dragCounter++;
  dropOverlay.classList.add("visible");
});

document.addEventListener("dragleave", (e) => {
  e.preventDefault();
  dragCounter--;
  if (dragCounter <= 0) {
    dragCounter = 0;
    dropOverlay.classList.remove("visible");
  }
});

document.addEventListener("dragover", (e) => {
  e.preventDefault();
});

document.addEventListener("drop", (e) => {
  e.preventDefault();
  dragCounter = 0;
  dropOverlay.classList.remove("visible");

  const file = e.dataTransfer.files[0];
  if (file && file.name.endsWith(".iplc")) {
    loadIplcFile(file);
  } else if (file) {
    status.textContent = `Unsupported file type: ${file.name}. Expected .iplc`;
  }
});

// --- Display helpers ---

function displayResult(result) {
  if (result.ok) {
    previousValues = new Map();
    renderVariables(result.variables, result.scans_completed, "run");
    diagnosticsPanel.innerHTML = '<p class="placeholder">No diagnostics.</p>';
    status.textContent = `Ran ${result.scans_completed} scan cycle(s)`;
    activateTab("variables");
  } else if (result.diagnostics && result.diagnostics.length > 0) {
    renderDiagnostics(result.diagnostics);
    variablesPanel.innerHTML = '<p class="placeholder">Compilation failed.</p>';
    status.textContent = `${result.diagnostics.length} error(s)`;
    activateTab("diagnostics");
  } else if (result.error) {
    previousValues = new Map();
    renderVariables(result.variables || [], result.scans_completed, "run");
    diagnosticsPanel.innerHTML = `<p class="error-message">${escapeHtml(result.error)}</p>`;
    status.textContent = "Runtime error";
    activateTab("diagnostics");
  }
}

function displayRunResult(result, filename) {
  if (result.ok) {
    previousValues = new Map();
    renderVariables(result.variables, result.scans_completed, "run");
    diagnosticsPanel.innerHTML = '<p class="placeholder">No diagnostics.</p>';
    status.textContent = `${filename}: ran ${result.scans_completed} scan cycle(s)`;
    activateTab("variables");
  } else {
    previousValues = new Map();
    renderVariables(result.variables || [], result.scans_completed, "run");
    diagnosticsPanel.innerHTML = `<p class="error-message">${escapeHtml(result.error || "Unknown error")}</p>`;
    status.textContent = `${filename}: runtime error`;
    activateTab("diagnostics");
  }
}

function displayStepResult(result) {
  if (result.ok) {
    renderVariables(result.variables, result.total_scans, "step");
    diagnosticsPanel.innerHTML = '<p class="placeholder">No diagnostics.</p>';
    status.textContent = `Scan cycle ${result.total_scans} completed`;
    activateTab("variables");
  } else if (result.diagnostics && result.diagnostics.length > 0) {
    renderDiagnostics(result.diagnostics);
    variablesPanel.innerHTML = '<p class="placeholder">Compilation failed.</p>';
    status.textContent = `${result.diagnostics.length} error(s)`;
    activateTab("diagnostics");
  } else if (result.error) {
    renderVariables(result.variables || [], result.total_scans, "step");
    diagnosticsPanel.innerHTML = `<p class="error-message">${escapeHtml(result.error)}</p>`;
    status.textContent = "Runtime error";
    activateTab("diagnostics");
  }
}

function renderVariables(variables, scansCompleted, mode) {
  if (!variables || variables.length === 0) {
    variablesPanel.innerHTML = '<p class="placeholder">No variables.</p>';
    return;
  }

  let html = '<div class="scan-summary">';
  if (mode === "step") {
    html += `<span class="scan-count">Scan cycle ${scansCompleted} completed</span>`;
    html += '<span class="scan-hint">Click Step again to run another cycle and see values change.</span>';
  } else {
    html += `<span class="scan-count">Ran ${scansCompleted} scan cycle(s) from initial state</span>`;
    html += '<span class="scan-hint">Each scan runs the entire program once. Variables persist between scans.</span>';
  }
  html += '</div>';

  html += '<table class="var-table"><thead><tr><th>Index</th><th>Value</th></tr></thead><tbody>';
  for (const v of variables) {
    const prev = previousValues.get(v.index);
    const changed = prev !== undefined && prev !== v.value;
    html += `<tr${changed ? ' class="changed"' : ''}>`;
    html += `<td>var[${v.index}]</td><td>${v.value}</td>`;
    html += '</tr>';
  }
  html += "</tbody></table>";
  html += '<p class="raw-bytes-note">Note: Values are shown as raw bytes, not interpreted values. Float types will not display correctly.</p>';
  variablesPanel.innerHTML = html;

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

function uint8ArrayToBase64(bytes) {
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}
