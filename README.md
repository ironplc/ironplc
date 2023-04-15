# IronPLC

⚠ This project's capabilities are limited to a parser, semantic analyzer, and
Visual Studio Code Extension that are that are building blocks for a complete
IEC 61131-3 runtime and development environment.

IronPLC aims to be a SoftPLC written entirely in safe Rust for embedded
devices running programs written in the IEC 61131-3 language.

[![Build Status](https://github.com/garretfick/ironplc/workflows/Build%20and%20Test/badge.svg)](https://github.com/garretfick/ironplc/actions?query=workflow%3ABuild-and-Test)
[![Automated Releases Status](https://github.com/garretfick/ironplc/workflows/Publish%20IronPLC%20Releases/badge.svg)](https://github.com/garretfick/ironplc/actions?query=workflow%3APublish-IronPLC-Releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)
[![Crates.io - 0.1.1](https://img.shields.io/crates/v/ironplc-plc2x)](https://crates.io/crates/ironplc-plc2x)
[![Dependency status - 0.1.1](https://deps.rs/crate/ironplc-plc2x/0.1.1/status.svg)](https://deps.rs/crate/ironplc-plc2x/0.1.1)
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

* ✅ Implement a parsing strategy for a single IEC 61131-3 structured text files
* ✅ Implement a strategy for semantic analysis
* ✅ Implement a strategy for reporting errors
* ✅ Setup automated builds to produce high-quality weekly snapshots
* ✅ Parser progress so that most OSCAT files parse without error
* IN PROGRESS Implement language server protocol to integrate diagnostics with Visual Studio Code
* NOT STARTED Build documentation website
* NOT STARTED Implement a code formatter for structured text files
* NOT STARTED Parser completed so that all valid OSCAT files parse without error

## Usage

There are two components to IronPLC:

* the `ironplcc` compiler prototype that check IEC 61131-3 libraries for
syntactic and semantic correctness
* the IronPLC Visual Studio Code Extension that integrates `ironplcc` and IEC 61131-3 language into Visual Studio Code

Follow the steps below to setup these components on Windows 10.

### Install the `ironplcc` Compiler

Follow the steps below to install the IronPLC compiler:

1. Navigate to [IronPLC releases](https://github.com/garretfick/ironplc/releases)
1. From the releases page, download the latest snapshot for your system, for
   example `ironplc-release-windows.msi`
1. Run the downloaded installer

Follow the steps below to check a single file:

1. Open the Windows **Start** menu, then type **Windows PowerShell** and launch
   the PowerShell App (or a terminal of your choice)
1. Input the following in the terminal window, then press `Enter`:
   ```ironplcc check 'C:\Program Files\ironplc\examples\getting_started.st'```

The program completes indicating that the file is a valid IEC 61131-3 file.

### Install the Visual Studio Code Extension

This part is a work in progress.

Follow the steps below to install the Visual Studio Code Extension for IronPLC:

1. Navigate to [IronPLC releases](https://github.com/garretfick/ironplc/releases)
1. From the releases page, download the latest snapshot for your system, for
   example `ironplc-vscode-extension-release-windows.vsix`
1. In Visual Studio Code, type `Ctrl+Shift+P`, then type `Install from VISX...` and choose the matching item
1. In the dialog, choose the VISX file you downloaded earlier

Follow then steps below to check a single file from within Visual Studio Code

1. In Visual Studio Code, choose **File » Open File...**
1. In the **Open File** dialog, choose `C:\Program Files\ironplc\examples\getting_started.st`, then choose **Open**

## Contributing

Contributions are very welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Similar Projects

* [RuSTy](https://github.com/PLC-lang/rusty) - Structured text compiler written in Rust. RuSTy is further along but the LGPL and LGPL licenses are difficult for industrial uses.
* [Structured Text language Support](https://github.com/Serhioromano/vscode-st) - Structured text language support for Visual Studio Code.
* [Beremiz](https://beremiz.org/) - A Python-based PLC programming environment.
* [msr](https://github.com/slowtec/msr) - A Rust library for industrial automation.
* [ethercat-rs](https://github.com/birkenfeld/ethercat-rs) - An experimental Rust automation toolbox using the IgH (Etherlab) EtherCAT master.
* [rustmatic](https://github.com/NOP0/rustmatic) - Rustmatic is a thought experiment on creating a PLC-like environment in Rust.
