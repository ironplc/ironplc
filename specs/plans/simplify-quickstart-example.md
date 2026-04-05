# Plan: Simplify Quickstart Example

Rework the quickstart tutorial to use a single progressive doorbell example,
introduce the sense-control-actuate concept early, and get the user to
compile and run code quickly.

## VM improvements

- Named + typed variable dump output (`Buzzer: TRUE` instead of `var[0]: 1`)
- `--dump-vars` defaults to stdout when no file path is given

## Quickstart structure

1. Installation (minor edits)
2. How a PLC Program Works (concept-only, sense-control-actuate cycle)
3. Your First Program (doorbell with BOOL, compile, run, see results)
4. Configuring Your Application (add CONFIGURATION + TON timer)
5. Working with Multiple Files (split program and config)
6. Connecting to Hardware (AT variables, check-only for now)
