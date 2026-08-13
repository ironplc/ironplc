// Hand-written ambient declarations for the wasm-pack-generated module at
// `./pkg/ironplc_playground.js`. The compiler crate is built with
// `--no-typescript` so no `.d.ts` is emitted upstream; this file mirrors the
// exported surface that `worker.ts` consumes.

declare module "*/pkg/ironplc_playground.js" {
  /** Initializes the WASM module. Returns a promise resolved after load. */
  export default function init(): Promise<unknown>;

  /** Installs a panic hook that forwards Rust panics to console.error. */
  export function init_panic_hook(): void;

  /**
   * Returns a JSON-encoded compilation result (diagnostics + bytecode).
   *
   * `libraries` is a JSON array of plain-text compatibility-library sources
   * (the served `.st` files the browser fetched) to activate, or `""` for none.
   */
  export function compile(
    source: string,
    dialect: string,
    allows: string,
    libraries: string,
  ): string;

  /** Runs previously compiled bytecode for `scans` cycles. Returns JSON. */
  export function run(bytecodeBase64: string, scans: number): string;

  /**
   * Compiles and runs source in one step. Returns JSON. `libraries` is a JSON
   * array of compatibility-library sources to activate, or `""` for none.
   */
  export function run_source(
    source: string,
    scans: number,
    dialect: string,
    allows: string,
    libraries: string,
  ): string;

  /**
   * Loads a program into the persistent session. Returns JSON. `libraries` is a
   * JSON array of compatibility-library sources to activate, or `""` for none.
   */
  export function load_program(
    source: string,
    cycleTimeUs: number,
    dialect: string,
    allows: string,
    libraries: string,
  ): string;

  /** Steps the current session by `scans` cycles. Returns JSON. */
  export function step(scans: number): string;

  /** Discards session state. Returns JSON. */
  export function reset_session(): string;

  /** Compiler version string. */
  export function version(): string;

  /** JSON-encoded array of selectable dialects ({ value, label, is_default }). */
  export function dialects(): string;
}
