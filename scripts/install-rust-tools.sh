#!/bin/bash
set -euo pipefail

# Installs Rust-based development tools required for building and testing
# the IronPLC compiler. Used by both the devcontainer Dockerfile and the
# Claude Code session-start hook.
#
# Usage: ./install-rust-tools.sh [--all] [INSTALL_DIR]
#   --all:        also install tools only needed for releases (cargo-release)
#   INSTALL_DIR:  where to place binaries (default: /usr/local/bin)

INSTALL_ALL=false
if [ "${1:-}" = "--all" ]; then
  INSTALL_ALL=true
  shift
fi

INSTALL_DIR="${1:-/usr/local/bin}"

# Install just (task runner) from pre-built binary
if ! command -v just &>/dev/null; then
  echo "Installing just..."
  JUST_VERSION=$(curl -sI "https://github.com/casey/just/releases/latest" 2>&1 \
    | grep -i "^location:" | sed 's|.*/tag/||' | tr -d '\r\n')
  curl -fsSL \
    "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xzf - -C "${INSTALL_DIR}" just
  echo "Installed $(just --version)"
fi

# Install cargo-llvm-cov (coverage tool) from pre-built binary
if ! cargo llvm-cov --version &>/dev/null; then
  echo "Installing cargo-llvm-cov..."
  LLVM_COV_VERSION=$(curl -sI "https://github.com/taiki-e/cargo-llvm-cov/releases/latest" 2>&1 \
    | grep -i "^location:" | sed 's|.*/tag/||' | tr -d '\r\n')
  curl -fsSL \
    "https://github.com/taiki-e/cargo-llvm-cov/releases/download/${LLVM_COV_VERSION}/cargo-llvm-cov-x86_64-unknown-linux-musl.tar.gz" \
    | tar xzf - -C "${INSTALL_DIR}"
  echo "Installed $(cargo llvm-cov --version)"
fi

# Install llvm-tools rustup component (required by cargo-llvm-cov)
if ! rustup component list --installed 2>/dev/null | grep -q llvm-tools; then
  echo "Installing llvm-tools rustup component..."
  rustup component add llvm-tools
fi

# Install cargo-release (version management) from source.
# Only installed with --all since it compiles from source (slow) and is
# only needed for release workflows, not the day-to-day CI pipeline.
if [ "${INSTALL_ALL}" = true ] && ! command -v cargo-release &>/dev/null; then
  echo "Installing cargo-release..."
  cargo install --root "$(dirname "${INSTALL_DIR}")" cargo-release
  echo "Installed cargo-release"
fi
