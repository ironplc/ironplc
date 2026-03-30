/** Generates the HTML for the Run Output webview panel.
 *
 *  The panel renders a variable table that updates in real time as the
 *  VM steps through scan cycles. Transport controls (Pause/Resume, Stop)
 *  are embedded in the panel header. A small inline sparkline is drawn
 *  for each numeric variable using a canvas element. */
export function getRunPanelHtml(): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>
    body {
      font-family: var(--vscode-font-family, sans-serif);
      font-size: var(--vscode-font-size, 13px);
      color: var(--vscode-editor-foreground);
      background: var(--vscode-editor-background);
      margin: 0;
      padding: 0;
    }
    .toolbar {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 8px 12px;
      border-bottom: 1px solid var(--vscode-panel-border, #333);
      background: var(--vscode-sideBar-background, transparent);
    }
    .toolbar button {
      background: var(--vscode-button-background);
      color: var(--vscode-button-foreground);
      border: none;
      padding: 4px 10px;
      cursor: pointer;
      border-radius: 2px;
      font-size: 12px;
    }
    .toolbar button:hover {
      background: var(--vscode-button-hoverBackground);
    }
    .toolbar .status {
      margin-left: auto;
      font-size: 11px;
      opacity: 0.7;
    }
    #error-banner {
      display: none;
      padding: 8px 12px;
      background: var(--vscode-inputValidation-errorBackground, #5a1d1d);
      color: var(--vscode-inputValidation-errorForeground, #f88);
      border-bottom: 1px solid var(--vscode-inputValidation-errorBorder, #be1100);
    }
    table {
      width: 100%;
      border-collapse: collapse;
    }
    th {
      text-align: left;
      padding: 6px 12px;
      border-bottom: 2px solid var(--vscode-panel-border, #333);
      font-weight: 600;
      font-size: 11px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
      opacity: 0.7;
    }
    td {
      padding: 4px 12px;
      border-bottom: 1px solid var(--vscode-panel-border, #222);
      font-family: var(--vscode-editor-fontFamily, monospace);
    }
    td.value {
      min-width: 80px;
    }
    td.value.changed {
      color: var(--vscode-charts-yellow, #e5c07b);
    }
    td.sparkline-cell {
      width: 120px;
      padding: 2px 8px;
    }
    canvas.sparkline {
      display: block;
    }
    .empty-state {
      padding: 24px;
      text-align: center;
      opacity: 0.5;
    }
  </style>
</head>
<body>
  <div class="toolbar">
    <button id="btn-pause" onclick="togglePause()">Pause</button>
    <button id="btn-stop" onclick="doStop()">Stop</button>
    <span class="status" id="status">Starting...</span>
  </div>
  <div id="error-banner"></div>
  <div id="content">
    <div class="empty-state">Waiting for program to start...</div>
  </div>

  <script>
    const vscode = acquireVsCodeApi();
    let isPaused = false;
    let previousValues = {};
    let valueHistory = {};
    const HISTORY_WINDOW_MS = 5000;

    function togglePause() {
      if (isPaused) {
        vscode.postMessage({ command: 'resume' });
      } else {
        vscode.postMessage({ command: 'pause' });
      }
    }

    function doStop() {
      vscode.postMessage({ command: 'stop' });
    }

    function parseNumericValue(value, typeName) {
      if (typeName === 'BOOL') return value === 'TRUE' ? 1 : 0;
      if (value.startsWith('16#')) return parseInt(value.substring(3), 16);
      if (value.startsWith('T#') || value.startsWith('D#') || value.startsWith('TOD#') || value.startsWith('DT#')) return NaN;
      const n = parseFloat(value);
      return isNaN(n) ? NaN : n;
    }

    function accumulateHistory(variables) {
      const now = performance.now();
      const cutoff = now - HISTORY_WINDOW_MS;

      for (const v of variables) {
        const num = parseNumericValue(v.value, v.type_name);
        if (isNaN(num)) continue;

        if (!valueHistory[v.index]) valueHistory[v.index] = [];
        const hist = valueHistory[v.index];
        hist.push({ t: now, v: num });

        // Prune old entries
        while (hist.length > 0 && hist[0].t < cutoff) hist.shift();
      }
    }

    function drawSparkline(canvas, history, isBool) {
      const ctx = canvas.getContext('2d');
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);

      if (history.length < 2) return;

      const now = performance.now();
      const windowStart = now - HISTORY_WINDOW_MS;
      const xs = history.map(e => ((e.t - windowStart) / HISTORY_WINDOW_MS) * w);
      const ys = history.map(e => e.v);

      let minY = Math.min(...ys);
      let maxY = Math.max(...ys);
      if (minY === maxY) { minY -= 1; maxY += 1; }
      const rangeY = maxY - minY;

      ctx.strokeStyle = '#6c8cff';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      for (let i = 0; i < history.length; i++) {
        const x = xs[i];
        const y = h - ((ys[i] - minY) / rangeY) * (h - 4) - 2;
        if (isBool && i > 0) {
          // Stepped line for booleans
          ctx.lineTo(x, h - ((ys[i - 1] - minY) / rangeY) * (h - 4) - 2);
        }
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }

    function renderVariables(result) {
      const container = document.getElementById('content');
      const variables = result.variables;

      if (!variables || variables.length === 0) {
        container.innerHTML = '<div class="empty-state">No variables</div>';
        return;
      }

      let html = '<table><tr><th>Variable</th><th>Type</th><th>Value</th><th>Trend</th></tr>';

      for (const v of variables) {
        const changed = previousValues[v.index] !== undefined && previousValues[v.index] !== v.value;
        const changedClass = changed ? ' changed' : '';
        const escapedName = v.name.replace(/</g, '&lt;');
        const escapedType = v.type_name.replace(/</g, '&lt;');
        const escapedValue = v.value.replace(/</g, '&lt;');

        html += '<tr>';
        html += '<td>' + escapedName + '</td>';
        html += '<td>' + escapedType + '</td>';
        html += '<td class="value' + changedClass + '">' + escapedValue + '</td>';
        html += '<td class="sparkline-cell"><canvas class="sparkline" data-index="' + v.index + '" data-type="' + escapedType + '" width="120" height="24"></canvas></td>';
        html += '</tr>';

        previousValues[v.index] = v.value;
      }

      html += '</table>';
      container.innerHTML = html;

      // Draw sparklines
      const canvases = container.querySelectorAll('canvas.sparkline');
      for (const canvas of canvases) {
        const idx = parseInt(canvas.dataset.index);
        const typeName = canvas.dataset.type;
        const hist = valueHistory[idx];
        if (hist && hist.length >= 2) {
          drawSparkline(canvas, hist, typeName === 'BOOL');
        }
      }
    }

    window.addEventListener('message', (event) => {
      const message = event.data;
      switch (message.type) {
        case 'started':
          document.getElementById('status').textContent = 'Running';
          document.getElementById('error-banner').style.display = 'none';
          isPaused = false;
          document.getElementById('btn-pause').textContent = 'Pause';
          break;
        case 'stopped':
          document.getElementById('status').textContent = 'Stopped';
          document.getElementById('btn-pause').textContent = 'Pause';
          isPaused = false;
          break;
        case 'paused':
          document.getElementById('status').textContent = 'Paused';
          document.getElementById('btn-pause').textContent = 'Resume';
          isPaused = true;
          break;
        case 'resumed':
          document.getElementById('status').textContent = 'Running';
          document.getElementById('btn-pause').textContent = 'Pause';
          isPaused = false;
          break;
        case 'variables':
          if (message.data.ok) {
            accumulateHistory(message.data.variables);
            renderVariables(message.data);
            document.getElementById('status').textContent =
              'Running \\u2022 Scan ' + message.data.total_scans;
          } else {
            document.getElementById('error-banner').textContent = message.data.error || 'VM error';
            document.getElementById('error-banner').style.display = 'block';
            document.getElementById('status').textContent = 'Faulted';
          }
          break;
        case 'error':
          document.getElementById('error-banner').textContent = message.message;
          document.getElementById('error-banner').style.display = 'block';
          document.getElementById('status').textContent = 'Error';
          break;
      }
    });
  </script>
</body>
</html>`;
}
