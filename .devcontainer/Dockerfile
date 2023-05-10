FROM mcr.microsoft.com/devcontainers/base:ubuntu
# Install the xz-utils package
RUN apt-get update && apt-get install -y rustc python3-sphinx python3-pip
RUN pip3 install furo sphinx-inline-tabs
RUN cargo install just
