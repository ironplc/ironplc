import init, { compile, run, run_source } from "./pkg/ironplc_wasm.js";

const editor = document.getElementById("editor");
const runBtn = document.getElementById("run-btn");
const scansInput = document.getElementById("scans-input");
const fileInput = document.getElementById("file-input");
const status = document.getElementById("status");
const variablesPanel = document.getElementById("variables-panel");
const diagnosticsPanel = document.getElementById("diagnostics-panel");
const dropOverlay = document.getElementById("drop-overlay");

// Tab switching
document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
    tab.classList.add("active");
    const panel = document.getElementById(`${tab.dataset.tab}-panel`);
    panel.classList.add("active");
  });
});

// Initialize WASM
init().then(() => {
  runBtn.disabled = false;
  status.textContent = "Ready";
});

// Compile & Run from editor
runBtn.addEventListener("click", () => {
  const source = editor.value;
  const scans = parseInt(scansInput.value, 10) || 1;

  status.textContent = "Compiling and running…";
  // Use setTimeout to allow the UI to update before blocking on wasm
  setTimeout(() => {
    const json = run_source(source, scans);
    const result = JSON.parse(json);
    displayResult(result);
  }, 10);
});

// Load .iplc file
fileInput.addEventListener("change", (e) => {
  const file = e.target.files[0];
  if (!file) return;
  loadIplcFile(file);
});

function loadIplcFile(file) {
  const reader = new FileReader();
  reader.onload = () => {
    const bytes = new Uint8Array(reader.result);
    const base64 = uint8ArrayToBase64(bytes);
    const scans = parseInt(scansInput.value, 10) || 1;

    status.textContent = `Running ${file.name}…`;
    setTimeout(() => {
      const json = run(base64, scans);
      const result = JSON.parse(json);
      displayRunResult(result, file.name);
    }, 10);
  };
  reader.readAsArrayBuffer(file);
}

// Drag and drop
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

function displayResult(result) {
  if (result.ok) {
    renderVariables(result.variables, result.scans_completed);
    diagnosticsPanel.innerHTML = '<p class="placeholder">No diagnostics.</p>';
    status.textContent = `Completed ${result.scans_completed} scan(s)`;
    activateTab("variables");
  } else if (result.diagnostics && result.diagnostics.length > 0) {
    renderDiagnostics(result.diagnostics);
    variablesPanel.innerHTML = '<p class="placeholder">Compilation failed.</p>';
    status.textContent = `${result.diagnostics.length} error(s)`;
    activateTab("diagnostics");
  } else if (result.error) {
    renderVariables(result.variables || [], result.scans_completed);
    diagnosticsPanel.innerHTML = `<p class="error-message">${escapeHtml(result.error)}</p>`;
    status.textContent = "Runtime error";
    activateTab("diagnostics");
  }
}

function displayRunResult(result, filename) {
  if (result.ok) {
    renderVariables(result.variables, result.scans_completed);
    diagnosticsPanel.innerHTML = '<p class="placeholder">No diagnostics.</p>';
    status.textContent = `${filename}: ${result.scans_completed} scan(s)`;
    activateTab("variables");
  } else {
    renderVariables(result.variables || [], result.scans_completed);
    diagnosticsPanel.innerHTML = `<p class="error-message">${escapeHtml(result.error || "Unknown error")}</p>`;
    status.textContent = `${filename}: runtime error`;
    activateTab("diagnostics");
  }
}

function renderVariables(variables, scansCompleted) {
  if (!variables || variables.length === 0) {
    variablesPanel.innerHTML = '<p class="placeholder">No variables.</p>';
    return;
  }

  let html = `<p class="success-message">Scans completed: ${scansCompleted}</p>`;
  html += '<table class="var-table"><thead><tr><th>Index</th><th>Value</th></tr></thead><tbody>';
  for (const v of variables) {
    html += `<tr><td>var[${v.index}]</td><td>${v.value}</td></tr>`;
  }
  html += "</tbody></table>";
  variablesPanel.innerHTML = html;
}

function renderDiagnostics(diagnostics) {
  let html = "";
  for (const d of diagnostics) {
    html += '<div class="diagnostic-item">';
    html += `<span class="diagnostic-code">${escapeHtml(d.code)}</span>`;
    html += `<span class="diagnostic-message">${escapeHtml(d.message)}</span>`;
    if (d.start > 0 || d.end > 0) {
      html += `<span class="diagnostic-location">offset ${d.start}–${d.end}</span>`;
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
