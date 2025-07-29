# Vectra Performance Benchmarks

This directory contains comprehensive benchmarks to compare the performance of the Rust implementation against the original Node.js vectra-enhanced library.

## Structure

- `rust/` - Rust benchmarks using the vectra crate
- `nodejs/` - Node.js benchmarks using vectra-enhanced 
- `data/` - Shared test data and vectors
- `results/` - Benchmark results and analysis
- `scripts/` - Automation scripts to run comparisons

## Benchmark Categories

### 1. Index Operations
- Index creation and initialization
- Index loading and validation
- Index deletion and cleanup

### 2. Item Operations  
- Single item insertion
- Batch item insertion (100, 1K, 10K items)
- Item retrieval by ID
- Item updates and modifications
- Item deletion

### 3. Vector Search Operations
- Single vector similarity search
- Batch vector searches
- Top-K queries (K=1, 10, 100)
- Filtered searches with metadata
- Range queries and pagination

### 4. Storage Format Tests
- Legacy JSON format compatibility
- Optimized v2 format performance
- Cross-format migration testing
- Storage size comparisons

### 5. Concurrency Tests
- Concurrent read operations
- Concurrent write operations
- Mixed read/write workloads
- Transaction performance

### 6. Scale Tests
- Small datasets (100-1K vectors)
- Medium datasets (10K-100K vectors) 
- Large datasets (100K+ vectors)
- High-dimensional vectors (128, 512, 1536 dims)

## Running Benchmarks

```bash
# Run all benchmarks
./scripts/run_all.sh

# Run specific category
./scripts/run_category.sh vector_search

# Compare implementations
./scripts/compare.sh --vectors 10000 --dimensions 512
```

## Test Data

All benchmarks use standardized test datasets:
- **Small**: 1K vectors, 128 dimensions
- **Medium**: 10K vectors, 384 dimensions  
- **Large**: 100K vectors, 512 dimensions
- **XL**: 1M vectors, 1536 dimensions (OpenAI embeddings size)