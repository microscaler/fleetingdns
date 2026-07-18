#!/bin/bash
# Run tests on ms02 after toolchain fix
set -e

cd /home/casibbald/Workspace/microscaler/fleetingdns

echo "=== FleetingDNS Test Runner on ms02 ==="
echo ""

# Check if rustc is available
if ! command -v rustc &> /dev/null; then
    echo "❌ rustc not found. Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
    source $HOME/.cargo/env
fi

echo "Current toolchain:"
rustc --version
echo ""

# Run the toolchain fix first
echo "Step 1: Running toolchain fix..."
./scripts/fix-toolchain.sh

echo ""
echo "Step 2: Testing edgehub..."
cargo test -p edgehub --lib -- --nocapture 2>&1 | tee /tmp/edgehub-tests.log

echo ""
echo "Step 3: Testing edf-cli..."
cargo test -p edf-cli --lib -- --nocapture 2>&1 | tee /tmp/edf-cli-tests.log

echo ""
echo "Step 4: Running E2E test..."
cargo test -p edgehub --test e2e_reverse_tunnel_http -- --nocapture 2>&1 | tee /tmp/e2e-tests.log

echo ""
echo "=== Test Results ==="
cat /tmp/edgehub-tests.log | tail -20
cat /tmp/edf-cli-tests.log | tail -20
cat /tmp/e2e-tests.log | tail -20
