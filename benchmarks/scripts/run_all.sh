#!/bin/bash

# Comprehensive benchmark runner for Vectra Rust vs Node.js comparison
set -e

# Default parameters
VECTORS=10000
DIMENSIONS=384
ITERATIONS=5
OUTPUT_DIR="../results"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Vectra Performance Benchmark Suite${NC}"
echo -e "${BLUE}=====================================${NC}"
echo ""

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--vectors)
            VECTORS="$2"
            shift 2
            ;;
        -d|--dimensions)
            DIMENSIONS="$2"  
            shift 2
            ;;
        -i|--iterations)
            ITERATIONS="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -v, --vectors N      Number of vectors to test (default: 10000)"
            echo "  -d, --dimensions N   Vector dimensions (default: 384)"
            echo "  -i, --iterations N   Number of iterations (default: 5)"
            echo "  -o, --output DIR     Output directory (default: ../results)"
            echo "  -h, --help          Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0 --vectors 5000 --dimensions 512"
            echo "  $0 --iterations 10 --output ./my_results"
            exit 0
            ;;
        *)
            echo -e "${RED}❌ Unknown option: $1${NC}"
            echo "Use -h or --help for usage information"
            exit 1
            ;;
    esac
done

echo -e "📊 ${YELLOW}Test Configuration:${NC}"
echo -e "   Vectors: ${VECTORS}"
echo -e "   Dimensions: ${DIMENSIONS}"
echo -e "   Iterations: ${ITERATIONS}"
echo -e "   Output: ${OUTPUT_DIR}"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Function to run benchmarks and handle errors
run_benchmark() {
    local impl="$1"
    local description="$2"
    local command="$3"
    
    echo -e "${BLUE}🔧 Running $description...${NC}"
    
    if eval "$command"; then
        echo -e "${GREEN}✅ $description completed successfully${NC}"
        return 0
    else
        echo -e "${RED}❌ $description failed${NC}"
        return 1
    fi
}

# Check if Rust benchmarks can be built
echo -e "${BLUE}🦀 Checking Rust environment...${NC}"
cd ../rust

if ! cargo check --quiet 2>/dev/null; then
    echo -e "${RED}❌ Rust benchmark compilation failed${NC}"
    echo -e "${YELLOW}   Make sure you have Rust installed and all dependencies available${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Rust environment ready${NC}"

# Run Rust benchmarks
echo ""
echo -e "${BLUE}🦀 Running Rust Benchmarks${NC}"
echo -e "${BLUE}=========================${NC}"

RUST_SUCCESS=true

run_benchmark "rust" "Rust Index Operations" \
    "cargo run --release -- --benchmark index --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || RUST_SUCCESS=false

run_benchmark "rust" "Rust Item Operations" \
    "cargo run --release -- --benchmark items --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || RUST_SUCCESS=false

run_benchmark "rust" "Rust Search Operations" \
    "cargo run --release -- --benchmark search --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || RUST_SUCCESS=false

run_benchmark "rust" "Rust Scale Tests" \
    "cargo run --release -- --benchmark scale --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || RUST_SUCCESS=false

# Check Node.js environment
echo ""
echo -e "${BLUE}🟢 Checking Node.js environment...${NC}"
cd ../nodejs

if ! node --version >/dev/null 2>&1; then
    echo -e "${RED}❌ Node.js not found${NC}"
    echo -e "${YELLOW}   Please install Node.js to run JavaScript benchmarks${NC}"
    NODE_SUCCESS=false
else
    echo -e "${GREEN}✅ Node.js environment ready${NC}"
    
    # Check if dependencies are installed
    if [ ! -d "node_modules" ]; then
        echo "📦 Installing Node.js dependencies..."
        npm install --silent
    fi
    
    # Run Node.js benchmarks
    echo ""
    echo -e "${BLUE}🟢 Running Node.js Benchmarks${NC}"
    echo -e "${BLUE}============================${NC}"
    
    NODE_SUCCESS=true
    
    run_benchmark "nodejs" "Node.js Index Operations" \
        "node index.js --benchmark index --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || NODE_SUCCESS=false
    
    run_benchmark "nodejs" "Node.js Item Operations" \
        "node index.js --benchmark items --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || NODE_SUCCESS=false
    
    run_benchmark "nodejs" "Node.js Search Operations" \
        "node index.js --benchmark search --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || NODE_SUCCESS=false
    
    run_benchmark "nodejs" "Node.js Scale Tests" \
        "node index.js --benchmark scale --vectors $VECTORS --dimensions $DIMENSIONS --iterations $ITERATIONS --output $OUTPUT_DIR" || NODE_SUCCESS=false
fi

# Generate comparison report
echo ""
echo -e "${BLUE}📊 Generating Comparison Report${NC}"
echo -e "${BLUE}==============================${NC}"

cd ../scripts

if [ -f "compare_results.py" ]; then
    if python3 compare_results.py "$OUTPUT_DIR" 2>/dev/null; then
        echo -e "${GREEN}✅ Comparison report generated${NC}"
    else
        echo -e "${YELLOW}⚠️  Could not generate comparison report (Python script may need dependencies)${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  Comparison script not found${NC}"
fi

# Summary
echo ""
echo -e "${BLUE}📈 Benchmark Summary${NC}"
echo -e "${BLUE}==================${NC}"

if [ "$RUST_SUCCESS" = true ]; then
    echo -e "${GREEN}✅ Rust benchmarks: SUCCESS${NC}"
else
    echo -e "${RED}❌ Rust benchmarks: FAILED${NC}"
fi

if [ "$NODE_SUCCESS" = true ]; then
    echo -e "${GREEN}✅ Node.js benchmarks: SUCCESS${NC}"
else
    echo -e "${RED}❌ Node.js benchmarks: FAILED or SKIPPED${NC}"
fi

echo ""
echo -e "${BLUE}📁 Results saved in: $OUTPUT_DIR${NC}"

# List generated files
if [ -d "$OUTPUT_DIR" ]; then
    echo -e "${BLUE}📄 Generated files:${NC}"
    ls -la "$OUTPUT_DIR"/*.json 2>/dev/null | while read -r line; do
        echo "   $line"
    done
fi

echo ""
echo -e "${GREEN}🎉 Benchmark suite completed!${NC}"

# Return appropriate exit code
if [ "$RUST_SUCCESS" = true ] && [ "$NODE_SUCCESS" = true ]; then
    exit 0
elif [ "$RUST_SUCCESS" = true ] || [ "$NODE_SUCCESS" = true ]; then
    exit 0
else
    exit 1
fi