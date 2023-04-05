# IronPLC

⚠ This project's capabilities are presently limited to a parser and
semantic analyzer that are building blocks for a complete IEC 61131-3 runtime.

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

IronPLC is nowhere near those capabilities yet.

## Mission

Complete runtime for IEC 61131-3 entirely in safe Rust and following
security best practices.

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

The current state of the project is it checks an IEC 61131-3 library for
syntactic and semantic correctness. The result is almost guaranteed to be
incorrect except for the most basic of libraries. You've been warned.

### Install

There exists an installer for Windows 10 and later.

Download a release from [IronPLC releases](https://github.com/garretfick/ironplc/releases)
then execute the installer.

Once installed, run the IEC 61131-3 checker on a small sample program generated
from [Beremiz](https://beremiz.org/), for example:

```cmd
ironplcc.exe check plc2x\resources\test\first_steps.st
```

### From Source

To run the checker, you need to install git, Rust and Cargo. Once you have
those, follow the steps below to check a library for correctness.

Get the code:

```sh
git clone https://github.com/garretfick/ironplc.git
cd ironplc
```

Run the checker using Cargo, for example on a sample program generated from
[Beremiz](https://beremiz.org/):

```sh
cargo run check plc2x\resources\test\first_steps.st
```

Alternatively, you can build the application then run the program directly:

```sh
cargo build
.\target\debug\ironplcc.exe check plc2x\resources\test\first_steps.st
```

## Developing

Compilers and runtimes are tricky to get right and hard to keep right. Use
Cargo to run tests during development:

```sh
cargo test
```

### Running the Full Test Suite

The `Cargo` test approach does not execute all tests. The full test suite
is defined in GitHub actions workflow. You can run the full tests locally
using [act](https://github.com/nektos/act) (requires Docker).

Follow the steps described in the [act](https://github.com/nektos/act)
repository to install `act`.

```sh
act --workflows ./.github/workflows/commit.yaml
```

### Debugging the Parser

The PEG parser is difficult to debug without a little help. The steps below
will help enormously in understanding and fixing what the parser is doing.

Run tests with the `trace` feature enabled to get output on rule matching
for any test that is failing:

```sh
cargo test --features trace
```

For even better debug support, use pegviz. First, build and install the pegviz
application into your path.

```sh
git clone https://github.com/fasterthanlime/pegviz.git
cd pegviz
cargo install --force --path .
```

After installing pegviz, pipe output to pegviz for pretty printing of results,
then open the generated file in a web browser.

```sh
cargo test --features trace | pegviz --output ./pegviz.html
```

## How It Works

The project is split into 3 parts:

* `dsl` defines relevant domain objects from the IEC 61131-3 language; it is
   the intermediate set of objects from parsing and contains an abstract syntax
   tree as one component (among many)
* `parser` is tokenizes and parses an IEC 61131-3 text file into the `dsl`
   objects
* `plc2x` is the front-end for a source-to-source compiler; it assembles all
   the pieces

There is no strict definition of what goes where. Better rules would be nice.

## Similar Projects

* [RuSTy](https://github.com/PLC-lang/rusty) - Structured text compiler written in Rust. RuSTy is further along but the LGPL and LGPL licenses are difficult for industrial uses.
* [msr](https://github.com/slowtec/msr) - A Rust library for industrial automation.
* [ethercat-rs](https://github.com/birkenfeld/ethercat-rs) - An experimental Rust automation toolbox using the IgH (Etherlab) EtherCAT master.
* [rustmatic](https://github.com/NOP0/rustmatic) - Rustmatic is a thought experiment on creating a PLC-like environment in Rust.
