#!/bin/sh
# IronPLC install script for Linux and macOS.
#
# Usage (one-liner):
#   curl -fsSL https://www.ironplc.com/install.sh | sh
#
# Install a specific version:
#   curl -fsSL https://www.ironplc.com/install.sh | IRONPLC_VERSION=v0.201.0 sh
#
# Flags:
#   --version <ver>      Release tag to install (also via IRONPLC_VERSION)
#   --install-dir <dir>  Install root (also via IRONPLC_INSTALL; default $HOME/.ironplc)
#   --no-modify-path     Do not touch shell profile files
#   --force              Reinstall even if the requested version is already present
#   -h, --help           Show help

set -eu

REPO="ironplc/ironplc"
RELEASE_URL="https://github.com/${REPO}/releases/download"
LATEST_API="https://api.github.com/repos/${REPO}/releases/latest"
LATEST_REDIRECT="https://github.com/${REPO}/releases/latest"
ISSUES_URL="https://github.com/${REPO}/issues"
DEFAULT_INSTALL_DIR="${HOME}/.ironplc"
# ironplcc is required. Older releases may not include ironplcvm or ironplcmcp.
REQUIRED_BINARIES="ironplcc"
OPTIONAL_BINARIES="ironplcvm ironplcmcp"

# ---- output helpers -------------------------------------------------------

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    BOLD="$(printf '\033[1m')"
    RED="$(printf '\033[31m')"
    GREEN="$(printf '\033[32m')"
    YELLOW="$(printf '\033[33m')"
    BLUE="$(printf '\033[34m')"
    RESET="$(printf '\033[0m')"
else
    BOLD=""; RED=""; GREEN=""; YELLOW=""; BLUE=""; RESET=""
fi

info()    { printf '%s==>%s %s\n' "${BLUE}${BOLD}" "${RESET}" "$*"; }
warn()    { printf '%swarning:%s %s\n' "${YELLOW}${BOLD}" "${RESET}" "$*" >&2; }
success() { printf '%s%s%s\n' "${GREEN}${BOLD}" "$*" "${RESET}"; }
error()   { printf '%serror:%s %s\n' "${RED}${BOLD}" "${RESET}" "$*" >&2; }
die()     { error "$*"; exit 1; }

usage() {
    cat <<'USAGE'
IronPLC install script

Usage:
  install.sh [--version <ver>] [--install-dir <dir>] [--no-modify-path] [--force]
  install.sh -h | --help

Options:
  --version <ver>      Release tag to install (e.g. v0.201.0 or 0.201.0).
                       Also accepts $IRONPLC_VERSION. Default: latest release.
  --install-dir <dir>  Install root (binaries go in <dir>/bin).
                       Also accepts $IRONPLC_INSTALL. Default: $HOME/.ironplc.
  --no-modify-path     Do not modify any shell profile files.
  --force              Reinstall even if the requested version is already present.
  -h, --help           Show this help.

Installs ironplcc, ironplcvm, and ironplcmcp from the latest IronPLC
GitHub release into $HOME/.ironplc/bin and (unless --no-modify-path)
adds that directory to your PATH via your shell profile.
USAGE
}

# ---- args / env -----------------------------------------------------------

VERSION_INPUT="${IRONPLC_VERSION:-}"
INSTALL_DIR="${IRONPLC_INSTALL:-${DEFAULT_INSTALL_DIR}}"
MODIFY_PATH=1
FORCE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --version)
            [ $# -ge 2 ] || die "--version requires an argument"
            VERSION_INPUT="$2"
            shift 2
            ;;
        --version=*)
            VERSION_INPUT="${1#--version=}"
            shift
            ;;
        --install-dir)
            [ $# -ge 2 ] || die "--install-dir requires an argument"
            INSTALL_DIR="$2"
            shift 2
            ;;
        --install-dir=*)
            INSTALL_DIR="${1#--install-dir=}"
            shift
            ;;
        --no-modify-path)
            MODIFY_PATH=0
            shift
            ;;
        --force)
            FORCE=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            error "unknown argument: $1"
            usage >&2
            exit 2
            ;;
    esac
done

# ---- prereq check ---------------------------------------------------------

have() { command -v "$1" >/dev/null 2>&1; }

need_cmd() {
    have "$1" || die "required command not found: $1"
}

need_cmd uname
need_cmd tar
need_cmd mkdir
need_cmd mktemp
need_cmd rm
need_cmd mv
need_cmd chmod

if have curl; then
    DOWNLOADER="curl"
elif have wget; then
    DOWNLOADER="wget"
else
    die "neither curl nor wget is available; please install one and retry"
fi

if have sha256sum; then
    SHA_TOOL="sha256sum"
elif have shasum; then
    SHA_TOOL="shasum"
elif have openssl; then
    SHA_TOOL="openssl"
else
    die "no SHA-256 tool found (need sha256sum, shasum, or openssl)"
fi

# ---- platform detection ---------------------------------------------------

detect_platform() {
    uname_s="$(uname -s)"
    uname_m="$(uname -m)"

    case "$uname_s" in
        Linux)   os="linux" ;;
        Darwin)  os="macos" ;;
        MINGW*|MSYS*|CYGWIN*)
            die "this script does not support Windows; download the Windows installer from ${RELEASE_URL}"
            ;;
        *)
            die "unsupported operating system: $uname_s"
            ;;
    esac

    case "$uname_m" in
        x86_64|amd64)         arch="x86_64" ;;
        arm64|aarch64)        arch="aarch64" ;;
        *)
            die "unsupported CPU architecture: $uname_m"
            ;;
    esac

    case "${os}-${arch}" in
        linux-x86_64)    artifact="ironplcc-x86_64-linux-musl.tar.gz" ;;
        macos-x86_64)    artifact="ironplcc-x86_64-macos.tar.gz" ;;
        macos-aarch64)   artifact="ironplcc-aarch64-macos.tar.gz" ;;
        linux-aarch64)
            die "Linux aarch64 (arm64) prebuilt binaries are not available yet. Track ${ISSUES_URL} or build from source."
            ;;
        *)
            die "unsupported platform combination: ${os}/${arch}"
            ;;
    esac

    PLATFORM_OS="$os"
    PLATFORM_ARCH="$arch"
    ARTIFACT_NAME="$artifact"
}

# ---- download helpers -----------------------------------------------------

download_to() {
    # download_to URL OUT_PATH
    _url="$1"; _out="$2"
    if [ "$DOWNLOADER" = "curl" ]; then
        curl --fail --location --retry 3 --retry-delay 2 \
             --silent --show-error \
             --output "$_out" "$_url"
    else
        wget --tries=3 --retry-connrefused --quiet -O "$_out" "$_url"
    fi
}

# Fetch a URL and print response body to stdout. Used for GitHub API.
fetch_stdout() {
    _url="$1"
    if [ "$DOWNLOADER" = "curl" ]; then
        curl --fail --location --retry 3 --retry-delay 2 \
             --silent --show-error "$_url"
    else
        wget --tries=3 --retry-connrefused --quiet -O - "$_url"
    fi
}

# Print the "Location" header target for a URL that 302-redirects. Used as a
# rate-limit-free fallback to the GitHub releases API.
resolve_redirect() {
    _url="$1"
    if [ "$DOWNLOADER" = "curl" ]; then
        curl --silent --show-error --head --location \
             --output /dev/null --write-out '%{url_effective}' "$_url"
    else
        # wget doesn't have a direct equivalent; use -S and parse Location.
        wget --server-response --spider --max-redirect=0 "$_url" 2>&1 \
            | sed -n 's/^  *[Ll]ocation: //p' | tail -n1
    fi
}

# ---- version resolution ---------------------------------------------------

resolve_version() {
    if [ -n "$VERSION_INPUT" ]; then
        case "$VERSION_INPUT" in
            v*) TAG="$VERSION_INPUT" ;;
            *)  TAG="v$VERSION_INPUT" ;;
        esac
        return
    fi

    info "resolving latest IronPLC release"
    _tag=""
    if _json="$(fetch_stdout "$LATEST_API" 2>/dev/null)"; then
        _tag="$(printf '%s' "$_json" \
            | grep -o '"tag_name"[^,]*' \
            | head -n1 \
            | sed -E 's/.*"tag_name"[^"]*"([^"]+)".*/\1/')"
    fi

    if [ -z "$_tag" ]; then
        warn "GitHub API lookup failed; falling back to redirect parsing"
        _effective="$(resolve_redirect "$LATEST_REDIRECT" 2>/dev/null || true)"
        _tag="$(printf '%s' "$_effective" | sed -n 's|.*/tag/\([^/?]*\).*|\1|p')"
    fi

    [ -n "$_tag" ] || die "could not determine the latest release tag; pass --version explicitly"
    TAG="$_tag"
}

# ---- checksum -------------------------------------------------------------

compute_sha256() {
    case "$SHA_TOOL" in
        sha256sum)
            sha256sum "$1" | awk '{print $1}'
            ;;
        shasum)
            shasum -a 256 "$1" | awk '{print $1}'
            ;;
        openssl)
            openssl dgst -sha256 "$1" | awk '{print $NF}'
            ;;
    esac
}

verify_checksum() {
    # verify_checksum file_path checksum_path
    _file="$1"; _cksum_file="$2"
    _expected="$(awk '{print $1; exit}' "$_cksum_file")"
    [ -n "$_expected" ] || die "checksum file is empty: $_cksum_file"
    _actual="$(compute_sha256 "$_file")"
    # Case-insensitive comparison.
    _expected_lc="$(printf '%s' "$_expected" | tr '[:upper:]' '[:lower:]')"
    _actual_lc="$(printf '%s' "$_actual" | tr '[:upper:]' '[:lower:]')"
    if [ "$_expected_lc" != "$_actual_lc" ]; then
        die "SHA-256 mismatch for $(basename "$_file") (expected $_expected, got $_actual)"
    fi
}

# ---- install flow ---------------------------------------------------------

already_installed_same_version() {
    _version_file="$INSTALL_DIR/VERSION"
    [ -f "$_version_file" ] || return 1
    _existing="$(cat "$_version_file" 2>/dev/null || true)"
    [ -n "$_existing" ] || return 1
    [ "$_existing" = "$TAG" ] || return 1
    for _bin in $REQUIRED_BINARIES; do
        [ -x "$INSTALL_DIR/bin/$_bin" ] || return 1
    done
    return 0
}

install_binaries() {
    _tmp="$(mktemp -d 2>/dev/null || mktemp -d -t ironplc)"
    # shellcheck disable=SC2064 # We want early expansion of $_tmp for the trap.
    trap "rm -rf \"$_tmp\"" EXIT INT TERM HUP

    _base="${RELEASE_URL}/${TAG}/${ARTIFACT_NAME}"
    info "downloading ${ARTIFACT_NAME} (${TAG})"
    download_to "$_base" "${_tmp}/${ARTIFACT_NAME}"
    download_to "${_base}.sha256" "${_tmp}/${ARTIFACT_NAME}.sha256"

    info "verifying checksum"
    verify_checksum "${_tmp}/${ARTIFACT_NAME}" "${_tmp}/${ARTIFACT_NAME}.sha256"

    info "extracting archive"
    tar -xzf "${_tmp}/${ARTIFACT_NAME}" -C "$_tmp"

    mkdir -p "${INSTALL_DIR}/bin"
    for _bin in $REQUIRED_BINARIES; do
        [ -f "${_tmp}/${_bin}" ] || die "archive is missing required binary: ${_bin}"
        mv -f "${_tmp}/${_bin}" "${INSTALL_DIR}/bin/${_bin}"
        chmod +x "${INSTALL_DIR}/bin/${_bin}"
    done
    for _bin in $OPTIONAL_BINARIES; do
        if [ -f "${_tmp}/${_bin}" ]; then
            mv -f "${_tmp}/${_bin}" "${INSTALL_DIR}/bin/${_bin}"
            chmod +x "${INSTALL_DIR}/bin/${_bin}"
        else
            warn "archive does not include ${_bin} (released before it existed); skipping"
        fi
    done

    # macOS may set com.apple.quarantine on extracted binaries. Best-effort removal.
    if [ "$PLATFORM_OS" = "macos" ] && have xattr; then
        xattr -dr com.apple.quarantine "${INSTALL_DIR}/bin" 2>/dev/null || true
    fi

    printf '%s\n' "$TAG" > "${INSTALL_DIR}/VERSION"
}

# ---- PATH / shell profile -------------------------------------------------

already_on_path() {
    # Returns 0 if "$INSTALL_DIR/bin" is already in $PATH.
    case ":${PATH}:" in
        *":${INSTALL_DIR}/bin:"*) return 0 ;;
        *) return 1 ;;
    esac
}

append_posix_block() {
    # append_posix_block <profile file>
    _profile="$1"
    # Write through a temporary file so we don't partial-write on error.
    {
        cat <<EOF

# >>> ironplc >>>
# Added by IronPLC install.sh. To remove, delete this block.
export IRONPLC_INSTALL="${INSTALL_DIR}"
case ":\$PATH:" in
    *":\$IRONPLC_INSTALL/bin:"*) ;;
    *) export PATH="\$IRONPLC_INSTALL/bin:\$PATH" ;;
esac
# <<< ironplc <<<
EOF
    } >> "$_profile"
    info "updated ${_profile}"
}

append_fish_block() {
    _profile="$1"
    mkdir -p "$(dirname "$_profile")"
    {
        cat <<EOF

# >>> ironplc >>>
# Added by IronPLC install.sh. To remove, delete this block.
set -gx IRONPLC_INSTALL "${INSTALL_DIR}"
fish_add_path -g "\$IRONPLC_INSTALL/bin"
# <<< ironplc <<<
EOF
    } >> "$_profile"
    info "updated ${_profile}"
}

profile_has_block() {
    # profile_has_block <file>
    [ -f "$1" ] && grep -q '^# >>> ironplc >>>' "$1"
}

configure_path() {
    if [ "$MODIFY_PATH" -eq 0 ]; then
        info "skipping PATH update (--no-modify-path)"
        return
    fi
    if already_on_path; then
        info "${INSTALL_DIR}/bin is already on \$PATH; not modifying any profile"
        return
    fi

    _updated=0
    for _f in "${HOME}/.bashrc" "${HOME}/.bash_profile" "${HOME}/.profile"; do
        [ -f "$_f" ] || continue
        if profile_has_block "$_f"; then
            info "${_f} already contains an ironplc block; leaving it in place"
            _updated=1
            continue
        fi
        append_posix_block "$_f"
        _updated=1
    done

    _zdotdir="${ZDOTDIR:-$HOME}"
    _zshrc="${_zdotdir}/.zshrc"
    if [ -f "$_zshrc" ] || [ "${SHELL##*/}" = "zsh" ]; then
        if profile_has_block "$_zshrc"; then
            info "${_zshrc} already contains an ironplc block; leaving it in place"
        else
            append_posix_block "$_zshrc"
        fi
        _updated=1
    fi

    _fish_config="${HOME}/.config/fish/config.fish"
    if [ -f "$_fish_config" ] || [ "${SHELL##*/}" = "fish" ]; then
        if profile_has_block "$_fish_config"; then
            info "${_fish_config} already contains an ironplc block; leaving it in place"
        else
            append_fish_block "$_fish_config"
        fi
        _updated=1
    fi

    if [ "$_updated" -eq 0 ]; then
        warn "no known shell profile found; add ${INSTALL_DIR}/bin to your PATH manually"
    fi
}

# ---- verification ---------------------------------------------------------

verify_install() {
    _bin="${INSTALL_DIR}/bin/ironplcc"
    [ -x "$_bin" ] || die "installation failed: $_bin is not executable"
    info "running: ironplcc version"
    "$_bin" version || die "ironplcc version failed"
}

print_next_steps() {
    success "IronPLC ${TAG} installed to ${INSTALL_DIR}/bin"
    cat <<EOF

To start using IronPLC in your current shell:

    export PATH="${INSTALL_DIR}/bin:\$PATH"

Or open a new terminal after reloading your shell profile. Then:

    ironplcc --help

Documentation: https://www.ironplc.com/quickstart/
EOF
}

# ---- main -----------------------------------------------------------------

main() {
    detect_platform
    resolve_version

    info "platform: ${PLATFORM_OS}/${PLATFORM_ARCH}"
    info "version:  ${TAG}"
    info "install:  ${INSTALL_DIR}"

    if [ "$FORCE" -eq 0 ] && already_installed_same_version; then
        info "${TAG} is already installed at ${INSTALL_DIR}; use --force to reinstall"
        verify_install
        print_next_steps
        return 0
    fi

    install_binaries
    configure_path
    verify_install
    print_next_steps
}

main "$@"
