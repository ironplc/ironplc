# Contributing

This component is the `ironplcc` compiler.

## Code Standards

The compiler follows specific architectural patterns and coding standards defined in the project's steering files:

* **Compiler Architecture** (`.kiro/steering/compiler-architecture.md`) - Module organization, semantic analysis patterns, and type system implementation
* **Problem Code Management** (`.kiro/steering/problem-code-management.md`) - Error handling patterns and diagnostic creation
* **IEC 61131-3 Compliance** (`.kiro/steering/iec-61131-3-compliance.md`) - Standard compliance validation and type system rules

These steering files provide detailed implementation guidance that complements the development workflow described below.

## Developing

Follow the steps in the sections below to setup and develop `ironplcc`.

### Prerequisites

If you are using the Dev Container , then you have everything you need. 
Otherwise, install git, just, Rust (stable) and Cargo. Get those from your preferred
source.

### Get the Code and Run a Test

```sh
git clone https://github.com/ironplc/ironplc.git
cd ironplc/compiler
```

Run the checker using Cargo:

```sh
cargo run check plc2x/resources/test/first_steps.st
```

### Making Changes

`ironplcc` has a large set of tests. Use `just` to execute the full build pipeline (compile, test, and lint):

```sh
just
```

You can also run individual tasks:
- `just compile` - Build the compiler
- `just test` - Run tests and check coverage
- `just lint` - Check code formatting and style

### Checking Coverage

The `ironplcc` development environment is set up to produce and
visualize test coverage in Visual Studio Code.

1. Use `just coverage` to produce the coverage information
1. Use commands `Coverage Gutters: Watch` or `Coverage Gutters: Display` to load the coverage into Visual Studio Code

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

The project is split into several parts. The best way to find out
what each part does is to open the Cargo.toml file and read the
description.

### Architecture Guidelines

When working on the compiler:

* **Keep modules focused**: Each analyzer/transformation module should handle one specific aspect and stay under 1000 lines of code
* **Follow semantic analysis patterns**: Use the `try_from` pattern for AST transformations and proper error handling
* **Implement proper diagnostics**: All errors must use the shared problem code system with clear, actionable messages
* **Maintain IEC 61131-3 compliance**: Follow standard compliance rules and support configurable validation levels

See the steering files referenced above for detailed implementation patterns and examples.
