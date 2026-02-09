#!/bin/bash
set -euo pipefail

# Only run in remote (web) environments
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

INSTALL_DIR="/usr/local/bin"

# Install just (task runner) from pre-built binary if not already available.
# Downloads from GitHub releases using the latest release tag.
if ! command -v just &>/dev/null; then
  echo "Installing just..."
  JUST_VERSION=$(curl -sI "https://github.com/casey/just/releases/latest" 2>&1 \
    | grep -i "^location:" | sed 's|.*/tag/||' | tr -d '\r\n')
  curl -fsSL \
    "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xzf - -C "${INSTALL_DIR}" just
  echo "Installed just $(just --version)"
fi

# Install cargo-llvm-cov (coverage tool) from pre-built binary if not already available.
# Required by the 'just coverage' recipe (compiler/justfile).
if ! cargo llvm-cov --version &>/dev/null; then
  echo "Installing cargo-llvm-cov..."
  LLVM_COV_VERSION=$(curl -sI "https://github.com/taiki-e/cargo-llvm-cov/releases/latest" 2>&1 \
    | grep -i "^location:" | sed 's|.*/tag/||' | tr -d '\r\n')
  curl -fsSL \
    "https://github.com/taiki-e/cargo-llvm-cov/releases/download/${LLVM_COV_VERSION}/cargo-llvm-cov-x86_64-unknown-linux-musl.tar.gz" \
    | tar xzf - -C "${INSTALL_DIR}"
  echo "Installed cargo-llvm-cov $(cargo llvm-cov --version)"
fi

# Install llvm-tools rustup component (required by cargo-llvm-cov for instrumentation)
if ! rustup component list --installed 2>/dev/null | grep -q llvm-tools; then
  echo "Installing llvm-tools rustup component..."
  rustup component add llvm-tools
fi
