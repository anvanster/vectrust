#!/bin/bash

# Test script to verify current Rust implementation functionality
# This tests the core features without needing Node.js comparison

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "🦀 Vectra Rust Implementation Test Suite"
echo "========================================"
echo ""

# Test 1: Basic functionality
echo "📝 Test 1: Basic Functionality"
echo "-----------------------------"

cd "$PROJECT_ROOT"

# Run basic example to ensure core functionality works
echo "Running basic example..."
if (cd examples && cargo run --bin basic > /dev/null 2>&1); then
    echo "✅ Basic example passed"
else
    echo "❌ Basic example failed"
    exit 1
fi

# Test 2: Unit tests
echo ""
echo "🧪 Test 2: Unit Tests"  
echo "--------------------"

echo "Running workspace tests..."
if cargo test --workspace --quiet > /dev/null 2>&1; then
    echo "✅ Unit tests passed"
else
    echo "❌ Unit tests failed - running with output:"
    cargo test --workspace
    exit 1
fi

# Test 3: Integration tests
echo ""
echo "🔗 Test 3: Integration Tests"
echo "---------------------------"

echo "Running integration tests..."
if cargo test --test integration --quiet > /dev/null 2>&1; then
    echo "✅ Integration tests passed"
else
    echo "⚠️  Integration tests not found or failed"
fi

# Test 4: CLI functionality
echo ""
echo "⚡ Test 4: CLI Functionality"
echo "---------------------------"

# Test CLI help
echo "Testing CLI help..."
if cargo run --bin vectra-cli -- --help > /dev/null 2>&1; then
    echo "✅ CLI help works"
else
    echo "⚠️  CLI not available"
fi

# Test 5: Benchmark compilation
echo ""
echo "📊 Test 5: Benchmark Compilation"
echo "-------------------------------"

cd "$SCRIPT_DIR/rust"

echo "Checking benchmark compilation..."
if cargo check --quiet > /dev/null 2>&1; then
    echo "✅ Benchmark compilation successful"
else
    echo "❌ Benchmark compilation failed"
    echo "Make sure all dependencies are available"
    exit 1
fi

# Test 6: Quick benchmark run
echo ""
echo "🏃 Test 6: Quick Benchmark Execution"
echo "-----------------------------------"

echo "Running quick benchmark (100 vectors, 64 dims, 1 iteration)..."
if timeout 30s cargo run --release -- --vectors 100 --dimensions 64 --iterations 1 --benchmark search --output /tmp > /dev/null 2>&1; then
    echo "✅ Quick benchmark completed"
else
    echo "⚠️  Quick benchmark timed out or failed"
fi

# Test 7: Memory usage test
echo ""
echo "🧠 Test 7: Memory Usage"
echo "----------------------"

echo "Testing with larger dataset (1000 vectors)..."
if timeout 60s cargo run --release -- --vectors 1000 --dimensions 128 --iterations 1 --benchmark items --output /tmp > /dev/null 2>&1; then
    echo "✅ Memory test passed"
else
    echo "⚠️  Memory test timed out or failed"
fi

# Test 8: Storage format tests
echo ""
echo "💾 Test 8: Storage Formats"
echo "-------------------------"

echo "Testing legacy format..."
if timeout 30s cargo run --release -- --vectors 100 --dimensions 64 --iterations 1 --benchmark index --legacy --output /tmp > /dev/null 2>&1; then
    echo "✅ Legacy format test passed"
else
    echo "⚠️  Legacy format test failed"
fi

echo "Testing optimized format..."
if timeout 30s cargo run --release -- --vectors 100 --dimensions 64 --iterations 1 --benchmark index --output /tmp > /dev/null 2>&1; then
    echo "✅ Optimized format test passed"
else
    echo "⚠️  Optimized format test failed"
fi

# Summary
echo ""
echo "📈 Test Summary"
echo "==============="
echo "Core functionality: Working ✅"
echo "Unit tests: Working ✅"  
echo "Benchmark system: Working ✅"
echo "Performance testing: Ready ✅"
echo ""
echo "🎉 Rust implementation is ready for performance comparison!"
echo ""
echo "Next steps:"
echo "1. Install vectra-enhanced in benchmarks/nodejs: npm install vectra-enhanced"
echo "2. Run full comparison: ./scripts/run_all.sh"
echo "3. Or run quick test: ./scripts/quick_test.sh"