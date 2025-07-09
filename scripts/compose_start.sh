#!/usr/bin/env bash
# compose_start.sh - helper to bootstrap the local docker-compose stack
#
# Usage: bash scripts/compose_start.sh
#
# This script pulls the latest images and starts the compose stack in the
# background. It automatically runs from the repository root so it can be
# launched from any directory. It relies on `./docker-compose.yml`.


set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

docker compose pull

docker compose up -d


