#!/bin/bash
# DNS verification test - checks actual state without modifying data
set -euo pipefail

echo "🔍 FleetingDNS Verification Test"
echo "================================"
echo "This test verifies the actual state without modifying any data"
echo ""

# Configuration
NETWORK="fleetingdns_default"
DNS_PORT="6353"

# Test function that ONLY reads, never writes
verify_dns() {
    local domain=$1
    local description=$2
    
    echo -n "Checking $domain ($description)... "
    
    # Get what's in Redis
    local redis_value=$(docker compose exec -T redis redis-cli GET slot:$domain 2>/dev/null || echo "NOT_IN_REDIS")
    redis_value=$(echo "$redis_value" | tr -d '\r\n')  # Remove any newlines
    if [ "$redis_value" == "(nil)" ] || [ -z "$redis_value" ]; then
        redis_value="NOT_IN_REDIS"
    fi
    
    # Get what DNS returns
    local dns_result=$(docker run --rm --network "$NETWORK" alpine sh -c "
        apk add --no-cache bind-tools >/dev/null 2>&1
        dig @dnsd -p $DNS_PORT $domain A +short 2>/dev/null || echo 'DNS_ERROR'
    " 2>/dev/null)
    
    if [ -z "$dns_result" ]; then
        dns_result="EMPTY_RESPONSE"
    fi
    
    # Show results
    echo ""
    echo "  Redis has: '$redis_value'"
    echo "  DNS returned: '$dns_result'"
    
    # Check consistency
    if [ "$redis_value" == "NOT_IN_REDIS" ] && [ "$dns_result" == "EMPTY_RESPONSE" ]; then
        echo "  ✅ Consistent: No data in Redis, DNS returns empty"
    elif [ "$redis_value" == "$dns_result" ]; then
        echo "  ✅ Consistent: Redis and DNS match"
    else
        echo "  ❌ INCONSISTENT: Redis and DNS don't match!"
    fi
    echo ""
}

# Run verifications
verify_dns "test.fdns.run" "Basic test domain"
verify_dns "app1.fdns.run" "Application 1"
verify_dns "app2.fdns.run" "Application 2"
verify_dns "webhook.fdns.run" "Webhook endpoint"
verify_dns "nonexistent.fdns.run" "Should not exist"
verify_dns "random-$(date +%s).fdns.run" "Random non-existent domain"

echo "================================"
echo "Verification complete!"