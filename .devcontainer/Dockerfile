FROM mcr.microsoft.com/devcontainers/base:ubuntu
# Install the Rust tools for the computer, Python and Sphinx for documentation
RUN apt-get update && apt-get install -y rustc cargo nodejs npm python3-sphinx python3-pip

# Install required Sphinx extensions for the documentation
RUN pip3 install furo sphinx-inline-tabs

# Install just, a task runner
RUN cargo install --root /usr/local just cargo-release llvm-conv

# Install typecript for the VS code extension
RUN npm install -g typescript
