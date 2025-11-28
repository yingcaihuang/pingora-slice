#!/bin/bash
# Stress testing script for Pingora Slice module

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Pingora Slice Stress Test ===${NC}"
echo ""

# Check if required tools are installed
command -v ab >/dev/null 2>&1 || { echo -e "${RED}Error: Apache Bench (ab) is required but not installed.${NC}" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { echo -e "${RED}Error: cargo is required but not installed.${NC}" >&2; exit 1; }

# Configuration
PROXY_PORT=${PROXY_PORT:-8080}
ORIGIN_PORT=${ORIGIN_PORT:-8081}
METRICS_PORT=${METRICS_PORT:-9090}
TEST_DURATION=${TEST_DURATION:-30}
CONCURRENT_CLIENTS=${CONCURRENT_CLIENTS:-50}

echo -e "${YELLOW}Configuration:${NC}"
echo "  Proxy Port: $PROXY_PORT"
echo "  Origin Port: $ORIGIN_PORT"
echo "  Metrics Port: $METRICS_PORT"
echo "  Test Duration: ${TEST_DURATION}s"
echo "  Concurrent Clients: $CONCURRENT_CLIENTS"
echo ""

# Build the project in release mode
echo -e "${YELLOW}Building project in release mode...${NC}"
cargo build --release
echo -e "${GREEN}✓ Build complete${NC}"
echo ""

# Function to cleanup background processes
cleanup() {
    echo -e "${YELLOW}Cleaning up...${NC}"
    pkill -P $$ || true
    exit
}
trap cleanup EXIT INT TERM

# Start a simple origin server (using Python)
echo -e "${YELLOW}Starting origin server on port $ORIGIN_PORT...${NC}"
python3 -m http.server $ORIGIN_PORT --directory /tmp &
ORIGIN_PID=$!
sleep 2
echo -e "${GREEN}✓ Origin server started (PID: $ORIGIN_PID)${NC}"
echo ""

# Create test files of various sizes
echo -e "${YELLOW}Creating test files...${NC}"
mkdir -p /tmp/test-files
dd if=/dev/urandom of=/tmp/test-files/1mb.bin bs=1M count=1 2>/dev/null
dd if=/dev/urandom of=/tmp/test-files/10mb.bin bs=1M count=10 2>/dev/null
dd if=/dev/urandom of=/tmp/test-files/100mb.bin bs=1M count=100 2>/dev/null
echo -e "${GREEN}✓ Test files created${NC}"
echo ""

# Start the Pingora Slice proxy
echo -e "${YELLOW}Starting Pingora Slice proxy...${NC}"
./target/release/pingora_slice &
PROXY_PID=$!
sleep 3
echo -e "${GREEN}✓ Proxy started (PID: $PROXY_PID)${NC}"
echo ""

# Function to run a test scenario
run_test() {
    local name=$1
    local file=$2
    local clients=$3
    local requests=$4
    
    echo -e "${YELLOW}Running test: $name${NC}"
    echo "  File: $file"
    echo "  Concurrent clients: $clients"
    echo "  Total requests: $requests"
    
    ab -n $requests -c $clients -g /tmp/ab_results.tsv \
        "http://localhost:$PROXY_PORT/test-files/$file" 2>&1 | \
        grep -E "(Requests per second|Time per request|Transfer rate|Failed requests)" || true
    
    echo ""
}

# Test Scenario 1: Small files (1MB)
echo -e "${GREEN}=== Scenario 1: Small Files (1MB) ===${NC}"
run_test "Small Files" "1mb.bin" 100 1000

# Test Scenario 2: Medium files (10MB)
echo -e "${GREEN}=== Scenario 2: Medium Files (10MB) ===${NC}"
run_test "Medium Files" "10mb.bin" 50 500

# Test Scenario 3: Large files (100MB)
echo -e "${GREEN}=== Scenario 3: Large Files (100MB) ===${NC}"
run_test "Large Files" "100mb.bin" 20 100

# Get metrics from the proxy
echo -e "${GREEN}=== Proxy Metrics ===${NC}"
if command -v curl >/dev/null 2>&1; then
    curl -s "http://localhost:$METRICS_PORT/metrics" || echo "Metrics endpoint not available"
else
    echo "curl not installed, skipping metrics"
fi
echo ""

# Memory usage
echo -e "${GREEN}=== Memory Usage ===${NC}"
if [ -d "/proc/$PROXY_PID" ]; then
    ps -p $PROXY_PID -o pid,vsz,rss,pmem,comm
else
    echo "Process information not available"
fi
echo ""

echo -e "${GREEN}=== Stress Test Complete ===${NC}"
echo ""
echo "Results saved to /tmp/ab_results.tsv"
echo "You can analyze the results using:"
echo "  - gnuplot for visualization"
echo "  - spreadsheet software for detailed analysis"
echo ""
