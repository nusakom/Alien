#!/bin/bash
# Simple TCP test server for testing Host client
# This simulates the kernel TCP server for basic testing

set -e

PORT=12345

echo "========================================"
echo "Simple TCP Test Server"
echo "========================================"
echo "Port: $PORT"
echo "Mode: Echo (simulates DBFS kernel)"
echo ""

# Check if netcat is available
if ! command -v nc &> /dev/null; then
    echo "❌ netcat (nc) not found!"
    echo "   Install: sudo apt install netcat"
    exit 1
fi

echo "✅ Starting server on port $PORT..."
echo "   Listening for connections..."
echo ""

# Simple echo server using netcat
while true; do
    echo "Waiting for connection..."
    nc -l -p $PORT
    echo "Connection closed"
    echo ""
done
