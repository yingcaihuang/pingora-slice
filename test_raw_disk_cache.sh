#!/bin/bash
# Test script for raw disk cache

set -e

echo "=== Raw Disk Cache Test Script ==="
echo

# Step 1: Create cache file
CACHE_FILE="./my-slice-raw"
CACHE_SIZE=$((1024 * 1024 * 1024))  # 1GB

echo "Step 1: Creating cache file..."
if [ -f "$CACHE_FILE" ]; then
    echo "  Cache file already exists, removing..."
    rm "$CACHE_FILE"
fi

# Use dd to create the file (works on all platforms)
echo "  Creating ${CACHE_SIZE} byte cache file..."
dd if=/dev/zero of="$CACHE_FILE" bs=1048576 count=1024 2>/dev/null
chmod 600 "$CACHE_FILE"
echo "  ✓ Cache file created: $(ls -lh $CACHE_FILE | awk '{print $5}')"
echo

# Step 2: Verify configuration
echo "Step 2: Verifying configuration..."
if [ ! -f "pingora_slice_raw_disk_full.yaml" ]; then
    echo "  ✗ Configuration file not found!"
    exit 1
fi
echo "  ✓ Configuration file found"
echo

# Step 3: Build the application
echo "Step 3: Building application..."
cargo build --release --quiet
echo "  ✓ Build complete"
echo

# Step 4: Run the application
echo "Step 4: Starting pingora-slice with raw disk cache..."
echo "  Configuration: pingora_slice_raw_disk_full.yaml"
echo "  Cache file: $CACHE_FILE"
echo "  Cache size: 1GB"
echo
echo "Press Ctrl+C to stop the server"
echo "=========================================="
echo

./target/release/pingora-slice pingora_slice_raw_disk_full.yaml
