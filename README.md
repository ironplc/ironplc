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

Complete runtime and development environment for IEC 61131-3. The runtime aims
to be written entirely in safe Rust to prevent security issues. The development
environment aims to be available via Visual Studio Code.

### Milestones

* ✅ Implement a parsing strategy for a single IEC 61131-3 structured text files
* ✅ Implement a strategy for semantic analysis
* ✅ Implement a strategy for reporting errors
* ✅ Setup automated builds to produce high-quality weekly snapshots
* IN PROGRESS Complete the parser so that all valid OSCAT files parse without error
* NOT STARTED Implement language server protocol to integrate diagnostics with Visual Studio Code
* NOT STARTED Build documentation website
* NOT STARTED Implement a code formatter for structured text files

## Usage

There are two components to IronPLC:

* `ironplcc`, a "compiler" that checks an IEC 61131-3 library for
syntactic and semantic correctness
* IronPLC Visual Studio Code Extension, an extension for Visual Studio code
  to work with IEC 61131-3 files and `ironplcc`

### Install the Compiler

There exists an installer for Windows 10 and later.

Download a release from [IronPLC releases](https://github.com/garretfick/ironplc/releases)
then execute the installer.

Once installed, run the IEC 61131-3 checker on a small sample program generated
from [Beremiz](https://beremiz.org/), for example:

```cmd
ironplcc.exe check plc2x\resources\test\first_steps.st
```

### Install the Visual Studio Code Extension

This part is a work in progress. Unfortunately there isn't yet a way to install
the extension.

## Similar Projects

* [RuSTy](https://github.com/PLC-lang/rusty) - Structured text compiler written in Rust. RuSTy is further along but the LGPL and LGPL licenses are difficult for industrial uses.
* [msr](https://github.com/slowtec/msr) - A Rust library for industrial automation.
* [ethercat-rs](https://github.com/birkenfeld/ethercat-rs) - An experimental Rust automation toolbox using the IgH (Etherlab) EtherCAT master.
* [rustmatic](https://github.com/NOP0/rustmatic) - Rustmatic is a thought experiment on creating a PLC-like environment in Rust.
