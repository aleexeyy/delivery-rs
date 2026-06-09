#!/usr/bin/env bash
# Starts a local OSRM routing server for the NYC area.
# Downloads and preprocesses OSM data on first run (~350 MB download, ~2 min processing).
# Data is cached in scripts/osrm/data/ — subsequent runs start instantly.
#
# Prerequisites: Docker must be running.
# Usage: ./scripts/run-osrm.sh
#
# The simulator reads OSRM_URL (default: http://localhost:5000) — just run this
# script before run-sim.sh and vehicles will follow real roads.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="${SCRIPT_DIR}/osrm/data"
OSM_PBF="${DATA_DIR}/new-york-latest.osm.pbf"
OSRM_BASE="${DATA_DIR}/new-york-latest.osrm"
OSRM_PORT="${OSRM_PORT:-5000}"
CONTAINER_NAME="delivery-rs-osrm"

# New York state extract from Geofabrik — covers the full NYC simulation area.
# ~350 MB download; processed files are kept in scripts/osrm/data/.
OSM_URL="https://download.geofabrik.de/north-america/us/new-york-latest.osm.pbf"

log() { echo "[$(date +'%H:%M:%S')] $*"; }

mkdir -p "${DATA_DIR}"

# ── 1. Download ──────────────────────────────────────────────────────────────
if [[ ! -f "${OSM_PBF}" ]]; then
    log "Downloading NYC OpenStreetMap data (~350 MB)…"
    curl -L --progress-bar "${OSM_URL}" -o "${OSM_PBF}"
    log "Download complete."
else
    log "OSM data already present — skipping download."
fi

# ── 2. Preprocess (extract + contract) ───────────────────────────────────────
if [[ ! -f "${OSRM_BASE}" ]]; then
    log "Extracting road graph (this takes ~1-2 minutes)…"
    docker run --rm \
        -v "${DATA_DIR}:/data" \
        osrm/osrm-backend \
        osrm-extract -p /opt/car.lua /data/new-york-latest.osm.pbf

    log "Contracting graph (this takes ~1-2 minutes)…"
    docker run --rm \
        -v "${DATA_DIR}:/data" \
        osrm/osrm-backend \
        osrm-contract /data/new-york-latest.osrm

    log "Preprocessing complete."
else
    log "Preprocessed graph already present — skipping preprocessing."
fi

# ── 3. Start server ───────────────────────────────────────────────────────────

# Remove any stale container with the same name
if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    log "Removing existing container '${CONTAINER_NAME}'…"
    docker rm -f "${CONTAINER_NAME}" > /dev/null
fi

log "Starting OSRM routing server on port ${OSRM_PORT} (Ctrl+C to stop)…"
docker run --rm \
    --name "${CONTAINER_NAME}" \
    -p "${OSRM_PORT}:5000" \
    -v "${DATA_DIR}:/data" \
    osrm/osrm-backend \
    osrm-routed --algorithm ch /data/new-york-latest.osrm
