#!/bin/bash

# FleetingDNS CI/CD Integration Test Script
# Non-interactive version for automated testing

set -euo pipefail  # Exit on error, undefined variables, pipe failures

# Configuration
REPORT_DIR="${REPORT_DIR:-./test-reports}"
DNS_PORT="${DNS_PORT:-6353}"
NETWORK="${DOCKER_NETWORK:-fleetingdns_default}"

# Initialize test tracking
OVERALL_STATUS="PASSED"
# Use simple variables instead of associative arrays for compatibility
TEST_BASIC_QUERY="PENDING"
TEST_MULTIPLE_DOMAINS="PENDING"
TEST_NONEXISTENT="PENDING"
TEST_PERFORMANCE="PENDING"
DURATION_BASIC_QUERY=0
DURATION_MULTIPLE_DOMAINS=0
DURATION_NONEXISTENT=0
DURATION_PERFORMANCE=0
START_TIME=$(date +%s.%N)

# Helper functions
log_info() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] INFO: $*"
}

log_error() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $*" >&2
}

run_dns_query() {
    local domain=$1
    local expected_ip=${2:-}
    
    local result=$(docker run --rm --network "$NETWORK" alpine sh -c "
        apk add --no-cache bind-tools > /dev/null 2>&1
        dig @dnsd -p $DNS_PORT $domain A +short 2>/dev/null
    " 2>/dev/null || echo "FAILED")
    
    echo "$result"
}

# Start tests
log_info "Starting FleetingDNS CI/CD Integration Tests"
log_info "Report directory: $REPORT_DIR"
mkdir -p "$REPORT_DIR"

# Check if DNS service is running
if ! docker compose ps | grep -q "dnsd.*running"; then
    log_info "Starting services..."
    docker compose up -d
    sleep 5  # Wait for services to start
fi

# Test 1: Basic DNS Query
log_info "Test 1: Basic DNS Query"
TEST_START=$(date +%s.%N)
docker compose exec -T redis redis-cli SET slot:test.fdns.run 192.168.1.100 > /dev/null
RESULT=$(run_dns_query "test.fdns.run" "192.168.1.100")
if [ "$RESULT" == "192.168.1.100" ]; then
    TEST_BASIC_QUERY="PASSED"
    log_info "✅ Basic query test passed"
else
    TEST_BASIC_QUERY="FAILED"
    OVERALL_STATUS="FAILED"
    log_error "❌ Basic query test failed. Expected: 192.168.1.100, Got: $RESULT"
fi
DURATION_BASIC_QUERY=$(echo "$(date +%s.%N) - $TEST_START" | bc)

# Test 2: Multiple Domains
log_info "Test 2: Multiple Domain Resolution"
TEST_START=$(date +%s.%N)
domains=("app1.fdns.run:10.0.0.1" "app2.fdns.run:10.0.0.2" "webhook.fdns.run:172.16.0.50")
ALL_PASSED=true

for domain_ip in "${domains[@]}"; do
    domain="${domain_ip%:*}"
    ip="${domain_ip#*:}"
    docker compose exec -T redis redis-cli SET slot:$domain $ip > /dev/null
    RESULT=$(run_dns_query "$domain" "$ip")
    if [ "$RESULT" != "$ip" ]; then
        ALL_PASSED=false
        log_error "❌ Failed to resolve $domain. Expected: $ip, Got: $RESULT"
    fi
done

if $ALL_PASSED; then
    TEST_MULTIPLE_DOMAINS="PASSED"
    log_info "✅ Multiple domain test passed"
else
    TEST_MULTIPLE_DOMAINS="FAILED"
    OVERALL_STATUS="FAILED"
fi
DURATION_MULTIPLE_DOMAINS=$(echo "$(date +%s.%N) - $TEST_START" | bc)

# Test 3: Non-existent Domain
log_info "Test 3: Non-existent Domain"
TEST_START=$(date +%s.%N)
RESULT=$(run_dns_query "nonexistent.fdns.run")
if [ -z "$RESULT" ] || [ "$RESULT" == "FAILED" ]; then
    TEST_NONEXISTENT="PASSED"
    log_info "✅ Non-existent domain test passed"
else
    TEST_NONEXISTENT="FAILED"
    OVERALL_STATUS="FAILED"
    log_error "❌ Non-existent domain test failed. Expected empty, Got: $RESULT"
fi
DURATION_NONEXISTENT=$(echo "$(date +%s.%N) - $TEST_START" | bc)

# Test 4: Performance Test
log_info "Test 4: Performance Test (10 queries)"
TEST_START=$(date +%s.%N)
PERF_FAILED=0
for i in {1..10}; do
    RESULT=$(run_dns_query "test.fdns.run")
    if [ "$RESULT" != "192.168.1.100" ]; then
        ((PERF_FAILED++))
    fi
    echo -n "."
done
echo ""

if [ $PERF_FAILED -eq 0 ]; then
    TEST_PERFORMANCE="PASSED"
    log_info "✅ Performance test passed (10/10 queries successful)"
else
    TEST_PERFORMANCE="FAILED"
    OVERALL_STATUS="FAILED"
    log_error "❌ Performance test failed ($PERF_FAILED/10 queries failed)"
fi
DURATION_PERFORMANCE=$(echo "$(date +%s.%N) - $TEST_START" | bc)

# Calculate total duration
END_TIME=$(date +%s.%N)
TOTAL_DURATION=$(echo "$END_TIME - $START_TIME" | bc)

# Generate JSON report
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
REPORT_FILE="$REPORT_DIR/dns-integration-test-$(date +%Y%m%d-%H%M%S).json"

cat > "$REPORT_FILE" << EOF
{
  "test_name": "FleetingDNS CI/CD Integration Test",
  "timestamp": "$TIMESTAMP",
  "status": "$OVERALL_STATUS",
  "total_duration_seconds": $TOTAL_DURATION,
  "tests": {
    "basic_dns_query": {
      "status": "$TEST_BASIC_QUERY",
      "duration_seconds": $DURATION_BASIC_QUERY,
      "description": "Test basic DNS query resolution"
    },
    "multiple_domains": {
      "status": "$TEST_MULTIPLE_DOMAINS",
      "duration_seconds": $DURATION_MULTIPLE_DOMAINS,
      "description": "Test multiple domain resolution"
    },
    "non_existent_domain": {
      "status": "$TEST_NONEXISTENT",
      "duration_seconds": $DURATION_NONEXISTENT,
      "description": "Test non-existent domain returns empty"
    },
    "performance": {
      "status": "$TEST_PERFORMANCE",
      "duration_seconds": $DURATION_PERFORMANCE,
      "description": "Test 10 rapid queries"
    }
  },
  "environment": {
    "docker_network": "$NETWORK",
    "dns_port": $DNS_PORT,
    "report_dir": "$REPORT_DIR"
  }
}
EOF

# Generate JUnit XML report for CI systems
JUNIT_FILE="$REPORT_DIR/dns-integration-junit.xml"
cat > "$JUNIT_FILE" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="FleetingDNS Integration Tests" time="$TOTAL_DURATION" tests="4" failures="$([[ $OVERALL_STATUS == "FAILED" ]] && echo "1" || echo "0")">
  <testsuite name="DNS Integration" tests="4">
    <testcase name="Basic DNS Query" classname="DNS.Integration" time="$DURATION_BASIC_QUERY"$([[ $TEST_BASIC_QUERY == "FAILED" ]] && echo ' status="failed"><failure message="DNS query failed"/></testcase>' || echo '/>')
    <testcase name="Multiple Domains" classname="DNS.Integration" time="$DURATION_MULTIPLE_DOMAINS"$([[ $TEST_MULTIPLE_DOMAINS == "FAILED" ]] && echo ' status="failed"><failure message="Multiple domain resolution failed"/></testcase>' || echo '/>')
    <testcase name="Non-existent Domain" classname="DNS.Integration" time="$DURATION_NONEXISTENT"$([[ $TEST_NONEXISTENT == "FAILED" ]] && echo ' status="failed"><failure message="Non-existent domain test failed"/></testcase>' || echo '/>')
    <testcase name="Performance Test" classname="DNS.Integration" time="$DURATION_PERFORMANCE"$([[ $TEST_PERFORMANCE == "FAILED" ]] && echo ' status="failed"><failure message="Performance test failed"/></testcase>' || echo '/>')
  </testsuite>
</testsuites>
EOF

# Summary
log_info "========================================="
log_info "Test Summary:"
log_info "  Status: $OVERALL_STATUS"
log_info "  Duration: ${TOTAL_DURATION}s"
log_info "  JSON Report: $REPORT_FILE"
log_info "  JUnit Report: $JUNIT_FILE"
log_info "========================================="

# Exit with appropriate code
if [ "$OVERALL_STATUS" == "PASSED" ]; then
    exit 0
else
    exit 1
fi