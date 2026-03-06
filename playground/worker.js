// Web Worker that loads the WASM module and runs compile/run operations
// off the main thread so the UI stays responsive.

import init, {
  init_panic_hook,
  compile,
  run,
  run_source,
  load_program,
  step,
  reset_session,
} from "./pkg/ironplc_playground.js";

let ready = false;

init()
  .then(() => {
    init_panic_hook();
    ready = true;
    self.postMessage({ type: "ready" });
  })
  .catch((err) => {
    self.postMessage({ type: "error", error: `WASM init failed: ${err}` });
  });

self.onmessage = (e) => {
  const { id, command, source, bytecodeBase64, scans } = e.data;

  if (!ready) {
    self.postMessage({
      id,
      type: "error",
      error: "WASM module not yet loaded",
    });
    return;
  }

  try {
    let json;
    switch (command) {
      case "compile":
        json = compile(source);
        break;
      case "run":
        json = run(bytecodeBase64, scans);
        break;
      case "run_source":
        json = run_source(source, scans);
        break;
      case "load_program":
        json = load_program(source);
        break;
      case "step":
        json = step(scans);
        break;
      case "reset":
        json = reset_session();
        break;
      default:
        self.postMessage({ id, type: "error", error: `Unknown command: ${command}` });
        return;
    }
    self.postMessage({ id, type: "result", json });
  } catch (err) {
    self.postMessage({ id, type: "error", error: String(err) });
  }
};
