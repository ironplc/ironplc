// Worker message contracts and shared data shapes for the playground.
//
// The browser thread (app.ts) sends WorkerRequest values to worker.ts, which
// dispatches into the WASM crate and posts back WorkerResponse. The WASM crate
// returns JSON strings that app.ts parses into RunResult / LoadResult.

// A dialect name (matching the compiler's `Dialect::cli_name`), or "" to use
// the playground default. The authoritative list of dialects comes from the
// WASM `dialects()` export (see DialectOption), so no enumeration is duplicated
// here — this alias just documents the contract.
export type Dialect = string;

// One selectable dialect, produced by the WASM `dialects()` export and used to
// build the dialect picker so it never drifts from the compiler.
export interface DialectOption {
  value: string;
  label: string;
  is_default: boolean;
}

export interface CompileRequest {
  id: number;
  command: "compile";
  source: string;
  dialect?: Dialect;
  allows?: string;
}

export interface RunRequest {
  id: number;
  command: "run";
  bytecodeBase64: string;
  scans: number;
}

export interface RunSourceRequest {
  id: number;
  command: "run_source";
  source: string;
  scans: number;
  dialect?: Dialect;
  allows?: string;
}

export interface LoadProgramRequest {
  id: number;
  command: "load_program";
  source: string;
  cycleTimeUs: number;
  dialect?: Dialect;
  allows?: string;
}

export interface StepRequest {
  id: number;
  command: "step";
  scans: number;
}

export interface ResetRequest {
  id: number;
  command: "reset";
}

export type WorkerRequest =
  | CompileRequest
  | RunRequest
  | RunSourceRequest
  | LoadProgramRequest
  | StepRequest
  | ResetRequest;

export interface ReadyMessage {
  type: "ready";
  version: string;
  dialects: DialectOption[];
}

export interface ErrorMessage {
  id?: number;
  type: "error";
  error: string;
}

export interface ResultMessage {
  id: number;
  type: "result";
  json: string;
}

export type WorkerResponse = ReadyMessage | ErrorMessage | ResultMessage;

export interface Variable {
  index: number;
  name?: string;
  type_name: string;
  value: string;
  valid?: boolean;
}

export interface Diagnostic {
  code: string;
  message: string;
  label?: string;
  /** Guidance on how to resolve the problem (e.g. "use `(* *)` comments"). */
  help?: string[];
  start_line: number;
  start_column: number;
  /** Compiler source file that produced the diagnostic (P9999/P9998 family). */
  compiler_file?: string;
  /** Compiler source line paired with `compiler_file`. */
  compiler_line?: number;
}

export interface RunResultOk {
  ok: true;
  total_scans: number;
  variables: Variable[];
}

export interface RunResultErr {
  ok: false;
  diagnostics?: Diagnostic[];
  error?: string;
  variables?: Variable[];
}

export type RunResult = RunResultOk | RunResultErr;
