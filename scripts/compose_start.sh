#!/opt/homebrew/Cellar/bash/5.2.37/bin/bash
# compose_start.sh - helper to bootstrap the local docker-compose stack
#
# Usage: bash scripts/compose_start.sh
#
# This script pulls the latest images and starts the compose stack in the
# background. Run from the repository root. It relies on `./docker-compose.yml`.


set -euo pipefail

docker compose pull

docker compose up -d


