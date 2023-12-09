# Use the typescript image because Typescript is the most challenging to setup.
FROM mcr.microsoft.com/devcontainers/typescript-node:18

# Install the Python and Sphinx for documentation
RUN apt-get update
RUN apt-get install -y python3-sphinx python3-pip curl xvfb libnss3 libatk1.0-0 libatk-bridge2.0-0 libdrm2 libgtk-3-0 libgbm1 libasound2 nsis

# Install required Sphinx extensions for the documentation
RUN pip3 install --break-system-packages furo sphinx-inline-tabs sphinx_copybutton esbonio

# Install Rust. This installs "globally" which is highly discouraged
# but we claim this is Ok because in this container we only have one user.
RUN export RUSTUP_HOME=/usr/local &&\
    export CARGO_HOME=/usr/local &&\
    curl https://sh.rustup.rs -sSf | sh -s -- -y

# Install the Rust-based tools. This is a bit risky because we use stable, and
# at the time of writing, stable updated every 6 weeks, so this doesn't
# guarantee the same version, but it does mean that we stay recent when the
# container is rebuilt (the same is generally true for most packages since we
# don't specify versions).
RUN rustup default stable
RUN cargo install --root /usr/local just
RUN cargo install --root /usr/local cargo-release
RUN cargo install --root /usr/local cargo-llvm-cov

# Install typecript for the VS code extension
RUN npm install -g typescript@5.1.6 vscode

RUN sudo --user=node rustup default stable
