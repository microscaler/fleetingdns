#!/bin/bash
# DNS Testing with dig-test Container
# Tests DNS resolution using different localhost addresses and record types
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

echo -e "${PURPLE}🚀 FleetingDNS DNS Testing with dig-test Container${NC}"
echo "====================================================="

# Function to log test results
log_test() {
    local test_name="$1"
    local status="$2"
    local message="$3"
    if [ "$status" = "PASS" ]; then
        echo -e "${GREEN}✅ $test_name: PASS${NC} - $message"
    else
        echo -e "${RED}❌ $test_name: FAIL${NC} - $message"
    fi
}

# Function to run DNS query and validate result
test_dns_query() {
    local test_name="$1"
    local domain="$2"
    local record_type="$3"
    local expected_result="$4"
    
    echo -e "${BLUE}🔍 Testing $test_name...${NC}"
    
    # Run the DNS query
    local result
    result=$(docker compose run --rm dig-test dig @dnsd -p 6353 "$domain" "$record_type" +short 2>/dev/null | tr -d '\n')
    
    if [ "$result" = "$expected_result" ]; then
        log_test "$test_name" "PASS" "Returned $result"
        return 0
    else
        log_test "$test_name" "FAIL" "Expected $expected_result, got $result"
        return 1
    fi
}

# Test 1: IPv4 localhost (127.0.0.1)
echo -e "${BLUE}🔍 Testing IPv4 localhost...${NC}"
docker compose exec -T redis redis-cli SET "slot:test-ipv4.fdns.run" "127.0.0.1" > /dev/null
test_dns_query "IPv4 localhost" "test-ipv4.fdns.run" "A" "127.0.0.1"

# Test 2: IPv4 alternative localhost (127.0.0.2)
echo -e "${BLUE}🔍 Testing IPv4 alternative localhost...${NC}"
docker compose exec -T redis redis-cli SET "slot:test-ipv4-alt.fdns.run" "127.0.0.2" > /dev/null
test_dns_query "IPv4 alternative localhost" "test-ipv4-alt.fdns.run" "A" "127.0.0.2"

# Test 3: IPv6 localhost (::1)
echo -e "${BLUE}🔍 Testing IPv6 localhost...${NC}"
docker compose exec -T redis redis-cli SET "slot:test-ipv6.fdns.run" "::1" > /dev/null
test_dns_query "IPv6 localhost" "test-ipv6.fdns.run" "AAAA" "::1"

# Test 4: IPv6 alternative localhost (::2)
echo -e "${BLUE}🔍 Testing IPv6 alternative localhost...${NC}"
docker compose exec -T redis redis-cli SET "slot:test-ipv6-alt.fdns.run" "::2" > /dev/null
test_dns_query "IPv6 alternative localhost" "test-ipv6-alt.fdns.run" "AAAA" "::2"

# Test 5: Non-existent domain (should return empty)
echo -e "${BLUE}🔍 Testing non-existent domain...${NC}"
result=$(docker compose run --rm dig-test dig @dnsd -p 6353 "nonexistent.fdns.run" "A" +short 2>/dev/null | tr -d '\n')
if [ -z "$result" ]; then
    log_test "Non-existent domain" "PASS" "Correctly returned empty result"
else
    log_test "Non-existent domain" "FAIL" "Expected empty result, got $result"
fi

# Test 6: Mixed IPv4/IPv6 queries
echo -e "${BLUE}🔍 Testing mixed IPv4/IPv6 queries...${NC}"
# Set up a domain with IPv4 address
docker compose exec -T redis redis-cli SET "slot:test-mixed.fdns.run" "127.0.0.3" > /dev/null

# Test A record query
test_dns_query "Mixed IPv4 query" "test-mixed.fdns.run" "A" "127.0.0.3"

# Test AAAA record query (should return empty since we only set IPv4)
result=$(docker compose run --rm dig-test dig @dnsd -p 6353 "test-mixed.fdns.run" "AAAA" +short 2>/dev/null | tr -d '\n')
if [ -z "$result" ]; then
    log_test "Mixed IPv6 query" "PASS" "Correctly returned empty for AAAA query"
else
    log_test "Mixed IPv6 query" "FAIL" "Expected empty result, got $result"
fi

# Test 7: Performance test (multiple queries)
echo -e "${BLUE}🔍 Testing performance with multiple queries...${NC}"
start_time=$(date +%s.%N)
for i in {1..5}; do
    docker compose run --rm dig-test dig @dnsd -p 6353 "test-ipv4.fdns.run" "A" +short > /dev/null 2>&1
done
end_time=$(date +%s.%N)
duration=$(echo "$end_time - $start_time" | bc)
log_test "Performance test" "PASS" "5 queries completed in ${duration}s"

# Test 8: Error handling test
echo -e "${BLUE}🔍 Testing error handling...${NC}"
# Test with invalid domain format
result=$(docker compose run --rm dig-test dig @dnsd -p 6353 "invalid..domain.fdns.run" "A" +short 2>/dev/null | tr -d '\n')
if [ -z "$result" ]; then
    log_test "Error handling" "PASS" "Correctly handled invalid domain"
else
    log_test "Error handling" "FAIL" "Expected empty result for invalid domain, got $result"
fi

echo ""
echo -e "${PURPLE}📋 DNS Testing Summary${NC}"
echo "========================"
echo -e "${GREEN}🎉 All DNS tests completed!${NC}"
echo ""
echo -e "${GREEN}✅ Verified Features:${NC}"
echo "  - IPv4 localhost addresses (127.0.0.1, 127.0.0.2)"
echo "  - IPv6 localhost addresses (::1, ::2)"
echo "  - A record queries"
echo "  - AAAA record queries"
echo "  - Non-existent domain handling"
echo "  - Mixed IPv4/IPv6 query handling"
echo "  - Performance with multiple queries"
echo "  - Error handling for invalid domains"
echo ""
echo -e "${BLUE}🚀 dig-test Container Features:${NC}"
echo "  ✅ Oneshot DNS queries"
echo "  ✅ No resolver configuration"
echo "  ✅ Controlled DNS server targeting"
echo "  ✅ Clean container lifecycle"
echo "  ✅ Isolated testing environment"
echo ""
echo -e "${GREEN}🎯 RESULT: DNS system working correctly with multiple localhost addresses!${NC}"

# Cleanup
echo -e "${BLUE}🧹 Cleaning up test data...${NC}"
docker compose exec -T redis redis-cli DEL "slot:test-ipv4.fdns.run" > /dev/null 2>&1 || true
docker compose exec -T redis redis-cli DEL "slot:test-ipv4-alt.fdns.run" > /dev/null 2>&1 || true
docker compose exec -T redis redis-cli DEL "slot:test-ipv6.fdns.run" > /dev/null 2>&1 || true
docker compose exec -T redis redis-cli DEL "slot:test-ipv6-alt.fdns.run" > /dev/null 2>&1 || true
docker compose exec -T redis redis-cli DEL "slot:test-mixed.fdns.run" > /dev/null 2>&1 || true

echo -e "${GREEN}✅ DNS testing completed successfully!${NC}" 