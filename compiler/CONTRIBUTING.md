# Contributing

This component is the `ironplcc` compiler.

## Developing

Follow the steps in the sections below to setup and develop `ironplcc`.

### Prerequisites

You need to install git, Rust (stable) and Cargo. Get those from your preferred
source.

### Get the Code and Run a Test

```sh
git clone https://github.com/garretfick/ironplc.git
cd ironplc/compiler
```

Run the checker using Cargo:

```sh
cargo run check plc2x\resources\test\first_steps.st
```

### Making Changes

`ironplcc` has an large set of tests. Use Cargo to execute them:

```sh
cargo test
cargo fmt
cargo clippy
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
