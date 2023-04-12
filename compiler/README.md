# `ironplcc`

`ironplcc` is the compiler for IronPLC, although it isn't a very good compiler
because it doesn't yet generate code.

## Developing

Follow the steps below to develop with `ironplcc`.

### From Source

To run the checker, you need to install git, Rust and Cargo. Once you have
those, follow the steps below to check a library for correctness.

Get the code:

```sh
git clone https://github.com/garretfick/ironplc.git
cd ironplc/compiler
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
