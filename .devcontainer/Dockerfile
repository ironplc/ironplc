# Use the typescript image because Typescript is the most challenging to setup.
# See https://github.com/devcontainers/images/tree/main/src/typescript-node for variants
FROM mcr.microsoft.com/devcontainers/typescript-node:22

# Install the Python and Sphinx for documentation. Combine update+install
# into a single RUN so the apt index is always fresh for the install step;
# splitting them lets Docker cache a stale index and then 404 when Debian
# rotates point-release packages.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        python3-venv python3-sphinx python3-pip curl xvfb libnss3 libatk1.0-0 \
        libatk-bridge2.0-0 libdrm2 libgtk-3-0 libgbm1 libasound2 nsis \
        gnupg lsb-release && \
    rm -rf /var/lib/apt/lists/*

# Install required Sphinx extensions for the documentation
RUN pip3 install --break-system-packages furo sphinx-inline-tabs sphinx_copybutton esbonio sphinx-design sphinx-sitemap sphinxext-opengraph

# Install Terraform from HashiCorp's apt repository (used by the
# infrastructure/ config to provision GitHub labels and the agent
# webhook). Tracks latest, same philosophy as the Rust tooling below.
RUN curl -fsSL https://apt.releases.hashicorp.com/gpg | gpg --dearmor -o /usr/share/keyrings/hashicorp-archive-keyring.gpg && \
    echo "deb [signed-by=/usr/share/keyrings/hashicorp-archive-keyring.gpg] https://apt.releases.hashicorp.com $(lsb_release -cs) main" > /etc/apt/sources.list.d/hashicorp.list && \
    apt-get update && \
    apt-get install -y --no-install-recommends terraform && \
    rm -rf /var/lib/apt/lists/*

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
