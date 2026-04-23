#!/usr/bin/env bash

# ==============================================================================
# Sovridium Tool: lint-docs.sh (Deterministic & Risk-Averse Edition)
# Purpose: High-integrity Markdown verification for the IronPLC Lattice.
# Principles: 
#   1. Risk-Aversion: Minimal system mutation; verify before action.
#   2. Deterministic Intent: Predictable paths; explicit human authorization.
# ==============================================================================

set -euo pipefail

# --- Colors for Terminal Output ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# --- Context Verification (Deterministic Context) ---
# Ensure we are running from the project root to prevent path ambiguity
if [ ! -f "justfile" ] || [ ! -d "docs" ]; then
    printf "%bError: Context Ambiguity Detected.%b\n" "${RED}" "${NC}"
    printf "This tool must be executed from the project root.\n"
    exit 1
fi

printf "%b%b🛰️  Sovridium Intelligence: Initiating Deterministic Documentation Audit...%b\n" "${BLUE}" "${BOLD}" "${NC}"

# --- OS Detection ---
OS="$(uname -s)"
case "${OS}" in
    Linux*)     OS_TYPE=Linux;;
    Darwin*)    OS_TYPE=Mac;;
    MINGW*|MSYS*|CYGWIN*) OS_TYPE=Windows;;
    *)          OS_TYPE="Unknown"
esac

# --- Dependency Check: Node.js ---
if ! command -v node > /dev/null 2>&1; then
    printf "%b%bCRITICAL: Runtime Missing%b\n" "${RED}" "${BOLD}" "${NC}"
    printf "Node.js is required to execute the linter logic.\n"
    printf "Intent: READ-ONLY. No system changes proposed.\n"
    exit 1
fi

# --- Dependency Check: markdownlint-cli ---
# Strategy: Check local node_modules first (Risk-Averse) then global path
LINTER_CMD=""
if [ -x "./node_modules/.bin/markdownlint" ]; then
    LINTER_CMD="./node_modules/.bin/markdownlint"
elif command -v markdownlint > /dev/null 2>&1; then
    LINTER_CMD="markdownlint"
fi

if [ -z "$LINTER_CMD" ]; then
    printf "\n"
    printf "%b%b⚠️  RISK ADVISORY: Missing Linting Substrate%b\n" "${YELLOW}" "${BOLD}" "${NC}"
    printf "%s\n" "----------------------------------------------------------------------"
    printf "The linter 'markdownlint-cli' is not found in local or global paths.\n"
    printf "\n"
    printf "%bDeterministic Intent:%b This tool defaults to NON-MUTATION.\n" "${BOLD}" "${NC}"
    printf "We recommend manual installation to maintain total system sovereignty.\n"
    printf "\n"
    printf "%bManual Installation Commands:%b\n" "${BOLD}" "${NC}"
    printf "  %bLocal (Recommended):%b npm install markdownlint-cli --no-save\n" "${CYAN}" "${NC}"
    printf "  %bGlobal (Requires Perms):%b npm install -g markdownlint-cli\n" "${CYAN}" "${NC}"
    printf "%s\n" "----------------------------------------------------------------------"
    printf "\n"

    if [ -t 0 ]; then
        printf "%bPlease select an installation path:%b\n" "${CYAN}" "${NC}"
        printf "  [1] %bLocal (Safest)%b - Installs to ./node_modules (no root required)\n" "${BOLD}" "${NC}"
        printf "  [2] %bGlobal%b - Installs system-wide (may require sudo)\n" "${BOLD}" "${NC}"
        printf "  [3] %bAbort%b - Exit without making any changes\n" "${BOLD}" "${NC}"
        printf "\n"
        read -p "Your choice [1-3]: " choice

        case $choice in
            1)
                printf "%bIntent: Local Installation (./node_modules)%b\n" "${BLUE}" "${NC}"
                npm install --prefix . markdownlint-cli --no-save
                LINTER_CMD="./node_modules/.bin/markdownlint"
                ;;
            2)
                printf "%bIntent: Global Installation%b\n" "${BLUE}" "${NC}"
                if [[ "$OS_TYPE" == "Linux" || "$OS_TYPE" == "Mac" ]]; then
                    printf "%bNote: System may prompt for your password for sudo permission.%b\n" "${YELLOW}" "${NC}"
                    sudo npm install -g markdownlint-cli || npm install -g markdownlint-cli
                else
                    npm install -g markdownlint-cli
                fi
                LINTER_CMD="markdownlint"
                ;;
            *)
                printf "%bAction: Passive Exit. No changes made.%b\n" "${YELLOW}" "${NC}"
                exit 1
                ;;
        esac
    else
        printf "%bError: Non-interactive environment. Cannot authorize mutation.%b\n" "${RED}" "${NC}"
        exit 1
    fi
fi

# --- Execution (Risk-Averse Audit) ---
# Second-pass verification of the binary path after potential installation
if [ -z "$LINTER_CMD" ] || [ ! -x "$(command -v "$LINTER_CMD" 2>/dev/null || echo "$LINTER_CMD")" ]; then
    # Final fallback check
    if [ -x "./node_modules/.bin/markdownlint" ]; then
        LINTER_CMD="./node_modules/.bin/markdownlint"
    elif command -v markdownlint > /dev/null 2>&1; then
        LINTER_CMD="markdownlint"
    else
        printf "%bError: Linter binary found but not executable or path ambiguity detected.%b\n" "${RED}" "${NC}"
        exit 1
    fi
fi

printf "%bIntent: RO-Audit (Execute Linter)%b\n" "${BLUE}" "${NC}"

# Verify target existence before execution
FILES_TO_LINT=$(find . -maxdepth 1 -name "*.md")
DOCS_TO_LINT=$(find docs -name "*.md" -not -path "docs/_build/*" 2>/dev/null || true)

if [ -z "$FILES_TO_LINT" ] && [ -z "$DOCS_TO_LINT" ]; then
    printf "%bWarning: No Markdown targets found for audit.%b\n" "${YELLOW}" "${NC}"
    exit 0
fi

# Execute with explicit config and strict error handling
if "$LINTER_CMD" *.md docs/**/*.md --ignore docs/_build --ignore node_modules --config .markdownlint.yaml; then
    printf "%b%b✅ Audit Success: Document Lattice is structurally sound.%b\n" "${GREEN}" "${BOLD}" "${NC}"
    exit 0
else
    printf "\n"
    printf "%b%b❌ Audit Failure: Inconsistencies detected.%b\n" "${RED}" "${BOLD}" "${NC}"
    exit 1
fi
