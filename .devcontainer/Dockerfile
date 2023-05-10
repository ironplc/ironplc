FROM mcr.microsoft.com/devcontainers/base:ubuntu
# Install the Rust tools for the computer, Python and Sphinx for documentation
RUN apt-get update && apt-get install -y rustc python3-sphinx python3-pip

# Install required Sphinx extensions for the documentation
RUN pip3 install furo sphinx-inline-tabs

# Install just, a task runner
RUN cargo install just
