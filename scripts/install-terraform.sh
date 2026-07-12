#!/bin/sh
# Install Terraform if it is not already on PATH.
#
# Shared by two environments that set up IronPLC dev tooling:
#   - The Claude Code remote/web environment, via the SessionStart hook in
#     .claude/settings.json (this env does not build the devcontainer image).
#   - Available for local use / CI when a devcontainer is not in play.
#
# The devcontainer (.devcontainer/Dockerfile) installs Terraform from
# HashiCorp's apt repo; this script is the apt-less equivalent for
# environments without that image. It downloads the official release binary,
# tracking latest (same "stay recent" philosophy as the Dockerfile). Pin a
# specific version by exporting TERRAFORM_VERSION.
#
# Idempotent: exits 0 immediately if terraform is already installed. Safe to
# run on every session start.
set -eu

if command -v terraform >/dev/null 2>&1; then
  echo "terraform already installed: $(terraform version | head -n1)"
  exit 0
fi

echo "Installing Terraform..."

# --- resolve version ---------------------------------------------------------
# Latest stable comes from the releases index (a public host reachable in both
# the devcontainer and the Claude proxy environment). Override with
# TERRAFORM_VERSION to pin. The checkpoint API is intentionally not used: it is
# blocked by the Claude agent proxy.
version="${TERRAFORM_VERSION:-}"
if [ -z "$version" ]; then
  version=$(curl -fsSL https://releases.hashicorp.com/terraform/index.json \
    | python3 -c '
import sys, json, re
d = json.load(sys.stdin)
vs = [v for v in d["versions"] if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", v)]
vs.sort(key=lambda s: [int(x) for x in s.split(".")])
print(vs[-1] if vs else "")')
fi
if [ -z "$version" ]; then
  echo "Warning: could not determine Terraform version." >&2
  exit 1
fi

# --- resolve os/arch for the release artifact --------------------------------
case "$(uname -s)" in
  Linux)  os=linux ;;
  Darwin) os=darwin ;;
  *) echo "Warning: unsupported OS $(uname -s) for Terraform install." >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64)  arch=amd64 ;;
  aarch64|arm64) arch=arm64 ;;
  *) echo "Warning: unsupported arch $(uname -m) for Terraform install." >&2; exit 1 ;;
esac

url="https://releases.hashicorp.com/terraform/${version}/terraform_${version}_${os}_${arch}.zip"

# --- install dir: prefer a system bin on PATH, else ~/.local/bin -------------
if [ -w /usr/local/bin ] || [ "$(id -u)" = "0" ]; then
  bindir=/usr/local/bin
else
  bindir="$HOME/.local/bin"
fi
mkdir -p "$bindir"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $url"
curl -fsSL -o "$tmp/terraform.zip" "$url"

# Unzip without assuming the unzip binary is present (Debian slim images omit
# it); fall back to Python's zipfile, which is always available here.
if command -v unzip >/dev/null 2>&1; then
  unzip -o -q "$tmp/terraform.zip" -d "$tmp"
else
  python3 -m zipfile -e "$tmp/terraform.zip" "$tmp"
fi

install -m 0755 "$tmp/terraform" "$bindir/terraform"

echo "Installed terraform ${version} to ${bindir}"
case ":$PATH:" in
  *":$bindir:"*) : ;;
  *) echo "Note: $bindir is not on PATH; add it to use terraform." >&2 ;;
esac
