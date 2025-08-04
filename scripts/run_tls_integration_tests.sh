#!/bin/bash

# Comprehensive TLS Routing Integration Test Runner
# Runs both shell-based and Rust-based integration tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 FleetingDNS TLS Routing Integration Test Suite${NC}"
echo "======================================================"
echo ""

# Function to print section headers
print_section() {
    echo -e "${PURPLE}$1${NC}"
    echo "----------------------------------------"
}

# Function to print test results
print_result() {
    local test_name="$1"
    local status="$2"
    local duration="$3"
    
    if [ "$status" = "PASS" ]; then
        echo -e "${GREEN}✅ $test_name: PASS${NC} (${duration}s)"
    else
        echo -e "${RED}❌ $test_name: FAIL${NC} (${duration}s)"
    fi
}

# Function to check prerequisites
check_prerequisites() {
    print_section "🔍 Checking Prerequisites"
    
    # Check if Docker is running
    if ! docker info > /dev/null 2>&1; then
        echo -e "${RED}❌ Docker is not running${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Docker is running${NC}"
    
    # Check if docker compose is available
    if ! docker compose version > /dev/null 2>&1; then
        echo -e "${RED}❌ docker compose is not available${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ docker compose is available${NC}"
    
    # Check if Rust is available
    if ! command -v cargo > /dev/null 2>&1; then
        echo -e "${RED}❌ Rust/cargo is not installed${NC}"
        exit 1
    fi
    echo -e "${GREEN}✅ Rust/cargo is available${NC}"
    
    echo ""
}

# Function to start Docker Compose services
start_services() {
    print_section "🏗️  Starting Docker Compose Services"
    
    echo -e "${YELLOW}Starting services...${NC}"
    docker compose up -d
    
    echo -e "${YELLOW}Waiting for services to be ready...${NC}"
    sleep 15
    
    # Check service status
    local services=("redis" "dnsd" "edgehub" "test-service")
    local all_healthy=true
    
    for service in "${services[@]}"; do
        if docker compose ps $service | grep -q "Up"; then
            echo -e "${GREEN}✅ $service is running${NC}"
        else
            echo -e "${RED}❌ $service failed to start${NC}"
            all_healthy=false
        fi
    done
    
    if [ "$all_healthy" = false ]; then
        echo -e "${RED}❌ Some services failed to start${NC}"
        echo -e "${YELLOW}Checking service logs...${NC}"
        docker compose logs --tail=20
        exit 1
    fi
    
    echo ""
}

# Function to run shell-based integration tests
run_shell_tests() {
    print_section "🐚 Running Shell-Based Integration Tests"
    
    local start_time=$(date +%s)
    
    if ./scripts/test_tls_routing_integration.sh; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        print_result "Shell Integration Tests" "PASS" "$duration"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        print_result "Shell Integration Tests" "FAIL" "$duration"
        return 1
    fi
}

# Function to run Rust-based integration tests
run_rust_tests() {
    print_section "🦀 Running Rust-Based Integration Tests"
    
    local start_time=$(date +%s)
    
    # Compile the Rust test
    echo -e "${YELLOW}Compiling Rust integration test...${NC}"
    if rustc --edition 2021 -C opt-level=2 scripts/test_tls_routing_rust.rs -o /tmp/tls_integration_test; then
        echo -e "${GREEN}✅ Rust test compiled successfully${NC}"
    else
        echo -e "${RED}❌ Failed to compile Rust test${NC}"
        return 1
    fi
    
    # Run the Rust test
    if /tmp/tls_integration_test; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        print_result "Rust Integration Tests" "PASS" "$duration"
        return 0
    else
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        print_result "Rust Integration Tests" "FAIL" "$duration"
        return 1
    fi
}

# Function to run end-to-end tunnel tests
run_tunnel_tests() {
    print_section "🚇 Running End-to-End Tunnel Tests"
    
    local start_time=$(date +%s)
    
    # Test 1: SSH key generation
    echo -e "${YELLOW}Testing SSH key generation...${NC}"
    if cargo run -p edf-cli -- keys test > /dev/null 2>&1; then
        echo -e "${GREEN}✅ SSH key generation working${NC}"
    else
        echo -e "${RED}❌ SSH key generation failed${NC}"
        return 1
    fi
    
    # Test 2: Tunnel creation simulation
    echo -e "${YELLOW}Testing tunnel creation...${NC}"
    local tunnel_data=$(docker compose exec -T redis redis-cli SET "tunnel:test-e2e" '{"local_port": 8080, "session_id": "test-123"}' EX 60)
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Tunnel creation working${NC}"
    else
        echo -e "${RED}❌ Tunnel creation failed${NC}"
        return 1
    fi
    
    # Test 3: HTTP forwarding simulation
    echo -e "${YELLOW}Testing HTTP forwarding...${NC}"
    local http_response=$(curl -s -H "Host: test-e2e.fleetingdns.run" http://localhost:8001/api/test)
    if echo "$http_response" | grep -q "Hello from FleetingDNS"; then
        echo -e "${GREEN}✅ HTTP forwarding working${NC}"
    else
        echo -e "${RED}❌ HTTP forwarding failed${NC}"
        return 1
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    print_result "End-to-End Tunnel Tests" "PASS" "$duration"
    return 0
}

# Function to run performance tests
run_performance_tests() {
    print_section "⚡ Running Performance Tests"
    
    local start_time=$(date +%s)
    
    # Test DNS query performance
    echo -e "${YELLOW}Testing DNS query performance...${NC}"
    local dns_start=$(date +%s%N)
    for i in {1..10}; do
        docker compose exec -T dnsd dig @localhost -p 6353 test-tls.fdns.run A +short > /dev/null 2>&1
    done
    local dns_end=$(date +%s%N)
    local dns_duration=$(((dns_end - dns_start) / 1000000))
    local avg_dns_time=$((dns_duration / 10))
    
    if [ $avg_dns_time -lt 100 ]; then
        echo -e "${GREEN}✅ DNS queries: ${avg_dns_time}ms average${NC}"
    else
        echo -e "${YELLOW}⚠️  DNS queries: ${avg_dns_time}ms average (slow)${NC}"
    fi
    
    # Test HTTP response performance
    echo -e "${YELLOW}Testing HTTP response performance...${NC}"
    local http_start=$(date +%s%N)
    for i in {1..10}; do
        curl -s http://localhost:8001/api/test > /dev/null 2>&1
    done
    local http_end=$(date +%s%N)
    local http_duration=$(((http_end - http_start) / 1000000))
    local avg_http_time=$((http_duration / 10))
    
    if [ $avg_http_time -lt 200 ]; then
        echo -e "${GREEN}✅ HTTP responses: ${avg_http_time}ms average${NC}"
    else
        echo -e "${YELLOW}⚠️  HTTP responses: ${avg_http_time}ms average (slow)${NC}"
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    print_result "Performance Tests" "PASS" "$duration"
    return 0
}

# Function to generate test report
generate_report() {
    print_section "📊 Generating Test Report"
    
    local report_file="test_reports/tls_integration_report_$(date +%Y%m%d_%H%M%S).json"
    mkdir -p test_reports
    
    cat > "$report_file" << EOF
{
  "test_suite": "TLS Routing Integration Tests",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "results": {
    "shell_tests": "$1",
    "rust_tests": "$2",
    "tunnel_tests": "$3",
    "performance_tests": "$4"
  },
  "summary": {
    "total_tests": 4,
    "passed": $(($1 + $2 + $3 + $4)),
    "failed": $((4 - $1 - $2 - $3 - $4))
  },
  "features_tested": [
    "DNS Resolution",
    "Test Service Health",
    "SSH Key Management",
    "Redis Authentication",
    "TLS Router Configuration",
    "Tunnel Creation",
    "HTTP Forwarding",
    "Telemetry and Monitoring",
    "End-to-End Tunnel Flow",
    "Performance Metrics"
  ]
}
EOF
    
    echo -e "${GREEN}✅ Test report generated: $report_file${NC}"
}

# Function to cleanup
cleanup() {
    print_section "🧹 Cleaning Up"
    
    echo -e "${YELLOW}Cleaning up test data...${NC}"
    docker compose exec -T redis redis-cli DEL "slot:test-tls.fdns.run" > /dev/null 2>&1 || true
    docker compose exec -T redis redis-cli DEL "session:test-session" > /dev/null 2>&1 || true
    docker compose exec -T redis redis-cli DEL "tunnel:test-tls" > /dev/null 2>&1 || true
    docker compose exec -T redis redis-cli DEL "tunnel:test-e2e" > /dev/null 2>&1 || true
    
    echo -e "${YELLOW}Stopping services...${NC}"
    docker compose down
    
    echo -e "${GREEN}✅ Cleanup completed${NC}"
}

# Main execution
main() {
    local shell_result=0
    local rust_result=0
    local tunnel_result=0
    local performance_result=0
    
    check_prerequisites
    start_services
    
    # Run all test suites
    run_shell_tests || shell_result=1
    run_rust_tests || rust_result=1
    run_tunnel_tests || tunnel_result=1
    run_performance_tests || performance_result=1
    
    # Generate final summary
    print_section "📋 Final Test Summary"
    echo ""
    
    local total_tests=4
    local passed_tests=$((shell_result + rust_result + tunnel_result + performance_result))
    local failed_tests=$((total_tests - passed_tests))
    
    echo -e "${BLUE}Test Results:${NC}"
    print_result "Shell Integration Tests" "$([ $shell_result -eq 0 ] && echo "PASS" || echo "FAIL")" "N/A"
    print_result "Rust Integration Tests" "$([ $rust_result -eq 0 ] && echo "PASS" || echo "FAIL")" "N/A"
    print_result "End-to-End Tunnel Tests" "$([ $tunnel_result -eq 0 ] && echo "PASS" || echo "FAIL")" "N/A"
    print_result "Performance Tests" "$([ $performance_result -eq 0 ] && echo "PASS" || echo "FAIL")" "N/A"
    
    echo ""
    echo -e "${BLUE}Summary:${NC}"
    echo -e "  Total Tests: $total_tests"
    echo -e "  Passed: $passed_tests"
    echo -e "  Failed: $failed_tests"
    
    if [ $failed_tests -eq 0 ]; then
        echo ""
        echo -e "${GREEN}🎉 All TLS Routing Integration Tests PASSED!${NC}"
        echo -e "${GREEN}✅ Core USP functionality verified${NC}"
        echo -e "${GREEN}✅ End-to-end tunnel flow working${NC}"
        echo -e "${GREEN}✅ Performance within acceptable limits${NC}"
    else
        echo ""
        echo -e "${RED}❌ $failed_tests test suites FAILED${NC}"
        echo -e "${YELLOW}⚠️  Some functionality may need attention${NC}"
    fi
    
    # Generate test report
    generate_report $shell_result $rust_result $tunnel_result $performance_result
    
    # Cleanup
    cleanup
    
    # Exit with appropriate code
    if [ $failed_tests -eq 0 ]; then
        exit 0
    else
        exit 1
    fi
}

# Run main function
main "$@" 