#!/bin/bash
# Simple non-interactive DNS integration test
set -euo pipefail

echo "🚀 FleetingDNS Simple Integration Test"
echo "======================================"

# Configuration
NETWORK="fleetingdns_default"
DNS_PORT="6353"
REPORT_DIR="./test-reports"
mkdir -p "$REPORT_DIR"

# Initialize counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Helper function to run test
run_test() {
    local test_name=$1
    local domain=$2
    local expected=$3
    
    ((TOTAL_TESTS++))
    
    echo -n "Testing $test_name ($domain -> $expected)... "
    
    # For tests that expect data, verify it exists first
    # Don't overwrite existing data - this allows us to test actual state
    if [ -n "$expected" ]; then
        local existing=$(docker compose exec -T redis redis-cli GET slot:$domain 2>/dev/null || echo "")
        if [ -z "$existing" ] || [ "$existing" == "(nil)" ]; then
            docker compose exec -T redis redis-cli SET slot:$domain $expected >/dev/null 2>&1
        fi
    fi
    
    # Run DNS query
    local result=$(docker run --rm --network "$NETWORK" alpine sh -c "
        apk add --no-cache bind-tools >/dev/null 2>&1
        dig @dnsd -p $DNS_PORT $domain A +short 2>/dev/null || echo 'ERROR'
    " 2>/dev/null)
    
    # Check result
    if [ "$result" == "$expected" ]; then
        echo "✅ PASSED"
        ((PASSED_TESTS++))
        return 0
    else
        echo "❌ FAILED (expected: '$expected', got: '$result')"
        ((FAILED_TESTS++))
        return 1
    fi
}

# Ensure services are running
echo "Checking services..."
if ! docker compose ps | grep -q "dnsd.*running"; then
    echo "Starting services..."
    docker compose up -d
    sleep 5
fi

echo ""
echo "Running tests..."
echo ""

# Run tests
run_test "Basic DNS Query" "test.fdns.run" "192.168.1.100" || true
run_test "Domain 1" "app1.fdns.run" "10.0.0.1" || true
run_test "Domain 2" "app2.fdns.run" "10.0.0.2" || true
run_test "Domain 3" "webhook.fdns.run" "172.16.0.50" || true
run_test "Non-existent Domain" "nonexistent.fdns.run" "" || true

# Generate simple report
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
REPORT_FILE="$REPORT_DIR/dns-simple-test-$(date +%Y%m%d-%H%M%S).txt"

cat > "$REPORT_FILE" << EOF
FleetingDNS Simple Integration Test Report
==========================================
Timestamp: $TIMESTAMP
Total Tests: $TOTAL_TESTS
Passed: $PASSED_TESTS
Failed: $FAILED_TESTS
Success Rate: $(( PASSED_TESTS * 100 / TOTAL_TESTS ))%

Environment:
- Docker Network: $NETWORK
- DNS Port: $DNS_PORT
EOF

echo ""
echo "======================================"
echo "Test Summary:"
echo "  Total: $TOTAL_TESTS"
echo "  Passed: $PASSED_TESTS"
echo "  Failed: $FAILED_TESTS"
echo "  Report: $REPORT_FILE"
echo "======================================"

# Exit with proper code
if [ $FAILED_TESTS -eq 0 ]; then
    exit 0
else
    exit 1
fi