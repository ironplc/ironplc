# Contributing

This component is the `ironplcc` compiler.

## Developing

Follow the steps in the sections below to setup and develop `ironplcc`.

### Prerequisites

If you are using the Dev Container , then you have everything you need. 
Otherwise, install git, just, Rust (stable) and Cargo. Get those from your preferred
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

`ironplcc` has an large set of tests. Use `just` to execute them:

```sh
just
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

The project is split into several parts:

* `dsl` defines relevant domain objects from the IEC 61131-3 language; it is
   the intermediate set of objects from parsing and contains an abstract syntax
   tree as one component (among many)
* `dsl-macro-dervice` defines macros used to implement `dsl`
* `parser` is tokenizes and parses an IEC 61131-3 text file into the `dsl`
   objects
* `plc2x` is the front-end for a source-to-source compiler; it assembles all
   the pieces
* `problems` defines the set of all possible compiler errors (uses code generation
   so that the problem codes are shared with the website).

There is no strict definition of what goes where. Better rules would be nice.
