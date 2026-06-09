#!/usr/bin/env bash
set -euo pipefail

# ----------------------------------------------------------------------
# Configuration
# ----------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="${SCRIPT_DIR}/.."
VENV_DIR="${PROJECT_ROOT}/scripts/sim/.venv"
SIMULATOR_PY="${PROJECT_ROOT}/scripts/sim/vehicles.py"
REQUIREMENTS_FILE="${PROJECT_ROOT}/scripts/sim/requirements.txt"

DEFAULT_VEHICLES=500

# ----------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------
log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*"
}

error_exit() {
    log "ERROR: $*" >&2
    exit 1
}

# ----------------------------------------------------------------------
# Parse number of vehicles
# ----------------------------------------------------------------------
NUM_VEHICLES="${NUM_VEHICLES:-}"
if [[ -z "${NUM_VEHICLES}" ]]; then
    if [[ $# -ge 1 ]]; then
        NUM_VEHICLES="$1"
    else
        NUM_VEHICLES="${DEFAULT_VEHICLES}"
    fi
fi

if ! [[ "${NUM_VEHICLES}" =~ ^[0-9]+$ ]] || [[ "${NUM_VEHICLES}" -lt 1 ]]; then
    error_exit "Invalid number of vehicles: '${NUM_VEHICLES}'. Must be a positive integer."
fi

log "Will simulate ${NUM_VEHICLES} vehicles."

# ----------------------------------------------------------------------
# Check prerequisites
# ----------------------------------------------------------------------
if ! command -v python3 &> /dev/null; then
    error_exit "python3 not found. Please install Python 3."
fi

if [[ ! -f "${SIMULATOR_PY}" ]]; then
    error_exit "Simulator script not found at ${SIMULATOR_PY}"
fi

# ----------------------------------------------------------------------
# Setup virtual environment (idempotent)
# ----------------------------------------------------------------------
if [[ ! -d "${VENV_DIR}" ]]; then
    log "Creating virtual environment at ${VENV_DIR}..."
    python3 -m venv "${VENV_DIR}" || error_exit "Failed to create venv."
else
    log "Virtual environment already exists at ${VENV_DIR}."
fi

# Activate venv
# shellcheck source=/dev/null
source "${VENV_DIR}/bin/activate"

# Upgrade pip
log "Upgrading pip..."
pip install --upgrade pip --quiet

# ----------------------------------------------------------------------
# Install dependencies if requirements.txt exists
# ----------------------------------------------------------------------
if [[ -f "${REQUIREMENTS_FILE}" ]]; then
    log "Installing dependencies from ${REQUIREMENTS_FILE}..."
    pip install --quiet -r "${REQUIREMENTS_FILE}" || error_exit "Failed to install dependencies."
else
    log "No requirements.txt found. Skipping dependency installation (assumes aiohttp already installed)."
    # Optionally, you could install aiohttp by default:
    # pip install --quiet aiohttp
fi

# ----------------------------------------------------------------------
# Run the simulator
# ----------------------------------------------------------------------
log "Starting simulator (press Ctrl+C to stop)..."
export SIM_NUM_VEHICLES="${NUM_VEHICLES}"
exec python "${SIMULATOR_PY}"
