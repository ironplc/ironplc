# IronPLC

⚠ This project's capabilities are limited to a parser, semantic analyzer, and
Visual Studio Code Extension that are that are building blocks for a complete
IEC 61131-3 runtime and development environment.

IronPLC aims to be a SoftPLC written entirely in safe Rust for embedded
devices running programs written in the IEC 61131-3 language.

[![IronPLC Integration](https://github.com/ironplc/ironplc/actions/workflows/integration.yaml/badge.svg)](https://github.com/ironplc/ironplc/actions/workflows/integration.yaml)
[![IronPLC Deployment](https://github.com/ironplc/ironplc/actions/workflows/deployment.yaml/badge.svg)](https://github.com/ironplc/ironplc/actions/workflows/deployment.yaml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)
[![Lines of Code](https://tokei.rs/b1/github/garretfick/ironplc)](https://github.com/XAMPPRocky/tokei)

SoftPLCs enable embedded and other computers to operate as programmable logic
controllers (PLCs) that execute all sorts of processes from home automation
and factories machines to industrial process and electrical power grid control.
PLC devices do this by implementing control algorithms that connect to sensors,
transducers and actuators through analog/digital IO, industrial protocols such as
I²C and Modbus, or even common internet protocol such as HTTP.

## Mission

A complete runtime and development environment for IEC 61131-3. The runtime aims
to be written entirely in safe Rust to prevent security issues. The development
environment aims to be available via Visual Studio Code to provide
a first class environment.

### Milestones

The project is progressing towards a minimum loveable product. The following
milestones are the current plan to achieve that loveable product.

* ✅ Implement a parsing strategy for a single IEC 61131-3 structured text file
* ✅ Implement a strategy for semantic analysis
* ✅ Implement a strategy for reporting errors
* ✅ Setup automated builds to produce high-quality weekly snapshots
* ✅ Parser progress so that most OSCAT files parse without error
* ✅ Implement language server protocol to integrate diagnostics with Visual Studio Code
* ✅ Build documentation website
* ✅ Build macOS installer
* NOT STARTED Parser completed so that all valid OSCAT files parse without error

## Usage

Go to [ironplc.com](https://www.ironplc.com) and follow the instructions
to get started.

## Contributing

Contributions are very welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Similar Projects

* [RuSTy](https://github.com/PLC-lang/rusty) - Structured text compiler written in Rust. RuSTy is further along but the LGPL and LGPL licenses are difficult for industrial uses.
* [Structured Text language Support](https://github.com/Serhioromano/vscode-st) - Structured text language support for Visual Studio Code.
* [Beremiz](https://beremiz.org/) - A Python-based PLC programming environment.
* [msr](https://github.com/slowtec/msr) - A Rust library for industrial automation.
* [ethercat-rs](https://github.com/birkenfeld/ethercat-rs) - An experimental Rust automation toolbox using the IgH (Etherlab) EtherCAT master.
* [rustmatic](https://github.com/NOP0/rustmatic) - Rustmatic is a thought experiment on creating a PLC-like environment in Rust.
