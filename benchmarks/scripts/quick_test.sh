#!/bin/bash

# Quick test script for smaller benchmarks during development
set -e

VECTORS=1000
DIMENSIONS=128
ITERATIONS=3

echo "🧪 Quick Vectra Performance Test"
echo "================================"
echo "Vectors: $VECTORS | Dimensions: $DIMENSIONS | Iterations: $ITERATIONS"
echo ""

# Test Rust implementation
echo "🦀 Testing Rust implementation..."
cd rust

if cargo run --release -- --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output ../results --benchmark search > /dev/null 2>&1; then
    echo "✅ Rust benchmark completed"
else
    echo "❌ Rust benchmark failed"
    exit 1
fi
cd ..

# Test Node.js implementation  
echo "🟢 Testing Node.js implementation..."
cd nodejs

if [ ! -d "node_modules" ]; then
    echo "📦 Installing dependencies..."
    npm install --silent
fi

if node index.js --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output ../results --benchmark search > /dev/null 2>&1; then
    echo "✅ Node.js benchmark completed"
else
    echo "❌ Node.js benchmark failed"
fi
cd ..

# Quick comparison
echo ""
echo "📊 Quick Results:"
cd ../scripts
python3 compare_results.py ../results --verbose

echo ""
echo "🎉 Quick test completed!"
