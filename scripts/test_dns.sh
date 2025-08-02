#!/bin/bash

# FleetingDNS End-to-End DNS Test Script
# This script tests DNS functionality from inside the Docker network

set -ex

echo "🚀 FleetingDNS End-to-End DNS Test"
echo "=================================="

# Check if docker compose is running
if ! docker compose ps | grep -q "dnsd.*running"; then
    echo "❌ DNS service is not running. Starting services..."
    docker compose up -d
    echo "⏳ Waiting for services to start..."
    sleep 5
fi

echo ""
echo "📋 Test 1: Basic DNS Query"
echo "--------------------------"
echo "Testing: test.fdns.run"

# Add test data to Redis
echo "➡️  Adding test data to Redis..."
docker compose exec -T redis redis-cli SET "slot:test.fdns.run" "192.168.1.100" > /dev/null
echo "✅ Added slot:test.fdns.run -> 192.168.1.100"

# Run DNS query from inside Docker network
echo ""
echo "➡️  Running DNS query..."
docker run --rm --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    echo '🔍 Query: dig @dnsd -p 6353 test.fdns.run A'
    echo ''
    dig @dnsd -p 6353 test.fdns.run A
"

echo ""
echo "📋 Test 2: Multiple Domain Test"
echo "-------------------------------"

# Add more test data
domains=(
    "app1.fdns.run:10.0.0.1"
    "app2.fdns.run:10.0.0.2"
    "webhook.fdns.run:172.16.0.50"
)

echo "➡️  Adding multiple test domains..."
for domain_ip in "${domains[@]}"; do
    domain="${domain_ip%:*}"
    ip="${domain_ip#*:}"
    docker compose exec -T redis redis-cli SET "slot:$domain" "$ip" > /dev/null
    echo "✅ Added slot:$domain -> $ip"
done

echo ""
echo "➡️  Testing all domains..."
for domain_ip in "${domains[@]}"; do
    domain="${domain_ip%:*}"
    echo ""
    echo "🔍 Testing $domain:"
    docker run --rm --network fleetingdns_default alpine sh -c "
        apk add --no-cache bind-tools > /dev/null 2>&1
        dig @dnsd -p 6353 $domain A +short
    "
done

echo ""
echo "📋 Test 3: Non-existent Domain"
echo "------------------------------"
echo "➡️  Testing non-existent domain (should return empty)..."
docker run --rm --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    echo '🔍 Query: dig @dnsd -p 6353 nonexistent.fdns.run A'
    dig @dnsd -p 6353 nonexistent.fdns.run A
"

echo ""
echo "📋 Test 4: Performance Test"
echo "---------------------------"
echo "➡️  Running 10 rapid DNS queries..."
echo ""

# Performance test with timing
start_time=$(date +%s.%N)
for i in {1..10}; do
    docker run --rm --network fleetingdns_default alpine sh -c '
        apk add --no-cache bind-tools > /dev/null 2>&1
        dig @dnsd -p 6353 test.fdns.run A +short > /dev/null
    ' 2>/dev/null
    echo -n "."
done
end_time=$(date +%s.%N)
duration=$(echo "$end_time - $start_time" | bc)
echo ""
echo "✅ Completed 10 queries in ${duration} seconds"

# Generate test report
REPORT_DIR="${REPORT_DIR:-./test-reports}"
mkdir -p "$REPORT_DIR"
REPORT_FILE="$REPORT_DIR/dns-integration-test-$(date +%Y%m%d-%H%M%S).json"

# Actually track ALL test results
TEST_STATUS="PASSED"

# Test 1 result
BASIC_QUERY_RESULT=$(docker run --rm --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    dig @dnsd -p 6353 test.fdns.run A +short 2>/dev/null || echo 'FAILED'
")
BASIC_QUERY_STATUS=$([[ "$BASIC_QUERY_RESULT" == "192.168.1.100" ]] && echo "PASSED" || echo "FAILED")
[[ "$BASIC_QUERY_STATUS" == "FAILED" ]] && TEST_STATUS="FAILED"

# Test 2 results - check each domain
MULTI_STATUS="PASSED"
APP1_RESULT=$(docker run --rm --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    dig @dnsd -p 6353 app1.fdns.run A +short 2>/dev/null || echo 'FAILED'
")
APP2_RESULT=$(docker run --rm --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    dig @dnsd -p 6353 app2.fdns.run A +short 2>/dev/null || echo 'FAILED'
")
WEBHOOK_RESULT=$(docker run --rm --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    dig @dnsd -p 6353 webhook.fdns.run A +short 2>/dev/null || echo 'FAILED'
")
[[ "$APP1_RESULT" != "10.0.0.1" || "$APP2_RESULT" != "10.0.0.2" || "$WEBHOOK_RESULT" != "172.16.0.50" ]] && MULTI_STATUS="FAILED" && TEST_STATUS="FAILED"

# Test 3 result
NONEXIST_RESULT=$(docker run --rm --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    dig @dnsd -p 6353 nonexistent.fdns.run A +short 2>/dev/null || echo ''
")
NONEXIST_STATUS=$([[ -z "$NONEXIST_RESULT" ]] && echo "PASSED" || echo "FAILED")
[[ "$NONEXIST_STATUS" == "FAILED" ]] && TEST_STATUS="FAILED"

# Test 4 - performance
PERF_COUNT=0
PERF_SUCCESS=0
for i in {1..10}; do
    RESULT=$(docker run --rm --network fleetingdns_default alpine sh -c "
        apk add --no-cache bind-tools > /dev/null 2>&1
        dig @dnsd -p 6353 test.fdns.run A +short 2>/dev/null
    " 2>/dev/null)
    [[ "$RESULT" == "192.168.1.100" ]] && ((PERF_SUCCESS++))
    ((PERF_COUNT++))
done
PERF_STATUS=$([[ $PERF_SUCCESS -eq $PERF_COUNT ]] && echo "PASSED" || echo "FAILED")
[[ "$PERF_STATUS" == "FAILED" ]] && TEST_STATUS="FAILED"

# Create JSON report with actual results
cat > "$REPORT_FILE" << EOF
{
  "test_name": "FleetingDNS Integration Test",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "status": "$TEST_STATUS",
  "duration_seconds": ${duration},
  "tests": {
    "basic_dns_query": {
      "status": "$BASIC_QUERY_STATUS",
      "domain": "test.fdns.run",
      "expected_ip": "192.168.1.100",
      "actual_ip": "$BASIC_QUERY_RESULT"
    },
    "multiple_domains": {
      "status": "$MULTI_STATUS",
      "domains_tested": {
        "app1.fdns.run": {"expected": "10.0.0.1", "actual": "$APP1_RESULT"},
        "app2.fdns.run": {"expected": "10.0.0.2", "actual": "$APP2_RESULT"},
        "webhook.fdns.run": {"expected": "172.16.0.50", "actual": "$WEBHOOK_RESULT"}
      }
    },
    "non_existent_domain": {
      "status": "$NONEXIST_STATUS",
      "domain": "nonexistent.fdns.run",
      "expected_response": "empty",
      "actual_response": "$NONEXIST_RESULT"
    },
    "performance": {
      "status": "$PERF_STATUS",
      "queries_count": $PERF_COUNT,
      "successful_queries": $PERF_SUCCESS,
      "total_duration_seconds": ${duration},
      "avg_query_time_seconds": $(echo "scale=4; ${duration} / 10" | bc)
    }
  },
  "environment": {
    "docker_network": "fleetingdns_default",
    "dns_port": 6353,
    "redis_backend": true
  }
}
EOF

echo ""
echo "📊 Test report written to: $REPORT_FILE"
echo ""
echo "✅ End-to-End DNS tests completed successfully!"
echo ""

# Exit with appropriate code based on test results
if [ -f "$REPORT_FILE" ]; then
    if [ "$TEST_STATUS" == "PASSED" ]; then
        echo "✅ All tests passed!"
        exit 0
    else
        echo "❌ Some tests failed! Check report: $REPORT_FILE"
        exit 1
    fi
else
    echo "❌ Failed to write test report"
    exit 2
fi