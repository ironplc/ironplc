# IronPLC

IronPLC aims to be a SoftPLC written entirely in "safe" Rust for embedded
devices running programs written in the IEC 61131-3 language.

[![Build Status](https://github.com/garretfick/ironplc/workflows/Build%20and%20Test/badge.svg)](https://github.com/garretfick/ironplc/actions?query=workflow%3ABuild-and-Test)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://opensource.org/licenses/MIT)
[![Crates.io - 0.1.1](https://img.shields.io/crates/v/ironplc-plc2x)](https://crates.io/crates/ironplc-plc2x)
[![Dependency status - 0.1.1](https://deps.rs/crate/ironplc-plc2x/0.1.1/status.svg)](https://deps.rs/crate/ironplc-plc2x/0.1.1)

SoftPLCs enable embedded and other computers to operate as programmable logic
controllers (PLCs) that execute all sorts of processes from home automation
and factories to industrial process andd electrical power grid control.
PLC-based devices do this by implementing control algorithms that connect to sensors,
transducers and actuators through analog/digital IO, industrial protocols such as
I²C and Modbus, or even common internet protocol such as HTTP.

IronPLC is now where near those capabilities yet. What exists is parser and semantic analyzer are building blocks for a
for IEC 61131-3. These are the first building blocks towards a complete runtime.

## Usage

The current state of the project is it checks an IEC 61131-3 library for
syntactic and semantic correctness. The result is almost guaranteed to be
incorrect except for the most basic of libraries.

To run the checker, you need to install git, Rust and Cargo. Once you have
those, follow the steps below to check a library for correctness.

Get the code:

```sh
git clone https://github.com/garretfick/ironplc.git
cd ironplc
```

Build the application:

```sh
cargo build
```

Run the IEC 61131-3 checker on a small sample program generated from
[Beremiz](https://beremiz.org/):

```sh
.\target\debug\ironplc-plc2x.exe plc2x\resources\test\first_steps.st
```

You can also run using Cargo directly:

```sh
cargo run plc2x\resources\test\first_steps.st
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

The project is split into 3 members:

* `dsl` defines relevant domain objects from the IEC 61131-3 language; it is
   the intermediate set of objects from parsing and contains an abstract syntax
   tree as one component (among many)
* `parser` is tokenizes and parses an IEC 61131-3 text file into the `dsl`
   objects
* `plc2x` is the front-end for a source-to-source compiler; it assembles all
   the pieces

There is no strict definition of what goes where. Better rules would be nice.
