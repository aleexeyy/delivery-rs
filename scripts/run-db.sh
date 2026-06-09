#!/usr/bin/env bash
set -euo pipefail

export DOCKER_BUILDKIT=1

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
if [ -f "${ROOT_DIR}/.env" ]; then
    set -a
    source "${ROOT_DIR}/.env"
    set +a
fi

POSTGRES_USER="${POSTGRES_USER:-admin}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-admin}"
POSTGRES_NAME="${POSTGRES_NAME:-fleet}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"

CONTAINER_NAME="postgres"
NETWORK_NAME="app-network"
LOCAL_DATA_DIR="$HOME/postgres_data"

# 1. Check and create network
if ! docker network inspect "${NETWORK_NAME}" >/dev/null 2>&1; then
    echo "Creating network: ${NETWORK_NAME}"
    docker network create "${NETWORK_NAME}"
else
    echo "Network ${NETWORK_NAME} already exists."
fi

# 2. Check and start container
if docker ps -a --format '{{.Names}}' | grep -Eq "^${CONTAINER_NAME}\$"; then
    echo "Container already exists. Starting it..."
    docker start "${CONTAINER_NAME}"
    exit 0
fi

if [ ! -d "${LOCAL_DATA_DIR}" ]; then
    mkdir -p "${LOCAL_DATA_DIR}"
fi

echo "Container does not exist. Creating it..."

docker run -d \
    --name "${CONTAINER_NAME}" \
    --network "${NETWORK_NAME}" \
    -e POSTGRES_USER="${POSTGRES_USER}" \
    -e POSTGRES_PASSWORD="${POSTGRES_PASSWORD}" \
    -e POSTGRES_DB="${POSTGRES_NAME}" \
    -p "${POSTGRES_PORT}:5432" \
    -v "${LOCAL_DATA_DIR}:/var/lib/postgresql/data" \
    postgres:16 -c listen_addresses='*'

echo "Postgres started on port ${POSTGRES_PORT}"
