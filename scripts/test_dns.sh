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
echo "📋 Test 4: Interactive Shell"
echo "----------------------------"
echo "➡️  Starting interactive shell with DNS tools..."
echo ""
echo "You can now run DNS queries manually:"
echo "  dig @dnsd -p 6353 test.fdns.run A"
echo "  dig @dnsd -p 6353 app1.fdns.run A"
echo "  nslookup test.fdns.run dnsd -port=6353"
echo ""
echo "Type 'exit' to quit"
echo ""

docker run --rm -it --network fleetingdns_default alpine sh -c "
    apk add --no-cache bind-tools > /dev/null 2>&1
    echo '🎯 Connected to FleetingDNS network. DNS server: dnsd:6353'
    echo ''
    /bin/sh
"

echo ""
echo "✅ End-to-End DNS tests completed!"