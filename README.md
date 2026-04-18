# IronPLC

![](docs/_static/ironplc-banner.svg)

An open-source toolchain and runtime for IEC 61131-3, written in safe Rust.

[![IronPLC Integration](https://github.com/ironplc/ironplc/actions/workflows/integration.yaml/badge.svg)](https://github.com/ironplc/ironplc/actions/workflows/integration.yaml)
[![IronPLC Deployment](https://github.com/ironplc/ironplc/actions/workflows/deployment.yaml/badge.svg)](https://github.com/ironplc/ironplc/actions/workflows/deployment.yaml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)

IronPLC is a compiler (`ironplcc`), runtime (`ironplcvm`), VS Code extension,
browser playground, and MCP server for writing and running IEC 61131-3
programs on off-the-shelf hardware — no proprietary IDE required.

## Quick Start

Install on Linux or macOS:

```sh
curl -fsSL https://www.ironplc.com/install.sh | sh
```

Or [see the installation docs](https://www.ironplc.com/quickstart/installation.html)
for Windows, Homebrew, and other options — or try it in the browser at
[playground.ironplc.com](https://playground.ironplc.com).

Then follow the [quick start tutorial](https://www.ironplc.com/quickstart/index.html)
to write your first PLC program.

## Capabilities

* [**Compiler**](https://www.ironplc.com/reference/compiler/index.html) —
  parses and analyzes IEC 61131-3 Structured Text with a large and growing
  set of diagnostic checks
* [**Runtime**](https://www.ironplc.com/reference/runtime/index.html) —
  executes compiled bytecode with task scheduling
* [**VS Code extension**](https://www.ironplc.com/reference/editor/overview.html) —
  syntax highlighting, real-time diagnostics, build tasks, and a bytecode viewer
* [**Multiple source formats**](https://www.ironplc.com/reference/compiler/source-formats/index.html) —
  Structured Text (`.st`, `.iec`), PLCopen XML (`.xml`), and TwinCAT
  (`.TcPOU`, `.TcGVL`, `.TcDUT`)
* [**AI agent support**](https://www.ironplc.com/how-to-guides/ai-agents/index.html) —
  an MCP server that lets tools like Claude call the IronPLC compiler
* [**Interactive playground**](https://playground.ironplc.com) —
  try IronPLC in the browser with no install

## What's in this repo

* `compiler/` — Rust workspace (compiler, runtime, MCP server, playground WASM)
* `integrations/vscode/` — VS Code extension
* `docs/` — Sphinx documentation site ([ironplc.com](https://www.ironplc.com))
* `playground/` — browser playground

## Contributing

Contributions are very welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Contact

[Create an issue](https://github.com/ironplc/ironplc/issues/new/choose) to reach out about IronPLC.

## Similar Projects

* [RuSTy](https://github.com/PLC-lang/rusty) - Structured text compiler written in Rust. RuSTy is further along but the LGPL and GPL licenses are difficult for industrial uses.
* [Structured Text language Support](https://github.com/Serhioromano/vscode-st) - Structured text language support for Visual Studio Code.
* [Beremiz](https://beremiz.org/) - A Python-based PLC programming environment.
* [RoboPLC](https://github.com/roboplc/roboplc/) - A Rust framework for creating industrial control appliances
* [msr](https://github.com/slowtec/msr) - A Rust library for industrial automation.
* [ethercat-rs](https://github.com/birkenfeld/ethercat-rs) - An experimental Rust automation toolbox using the IgH (Etherlab) EtherCAT master.
* [rustmatic](https://github.com/NOP0/rustmatic) - Rustmatic is a thought experiment on creating a PLC-like environment in Rust.
