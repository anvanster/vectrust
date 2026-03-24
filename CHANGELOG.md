# Changelog

All notable changes to Vectrust will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-03-24

### Added
- **Graph database**: Nodes, edges, properties, labels, and relationship types
- **Cypher query language**: Hand-written recursive descent parser with logos lexer
  - Data manipulation: CREATE, SET, DELETE, DETACH DELETE, REMOVE
  - Reading: MATCH, WHERE, RETURN, ORDER BY, LIMIT, SKIP, WITH
  - Variable-length paths: `MATCH (a)-[:REL*1..3]->(b)`
  - Aggregation: count(), collect(), sum(), avg(), min(), max() with automatic grouping
  - DISTINCT support
  - CALL...YIELD procedure syntax
- **Vector extensions to Cypher**: `vector_similarity()`, `vector_distance()` functions
- **kNN procedure**: `CALL vectrust.nearest('field', $query, k) YIELD node, score`
- **GraphIndex facade**: Unified API for graph + vector operations
  - Programmatic: `create_node()`, `create_edge()`, `create_node_with_vector()`
  - Cypher: `cypher()`, `cypher_with_params()`
  - Vector: `insert_vector()`, `get_vector()`, `query_vectors()`
  - Stats: `graph_stats()` returning node/edge counts, labels, relationship types
- **Shared RocksDB storage**: Single database instance with 11 column families for graph and vector data
- **Graph storage**: 9 column families (nodes, edges, adjacency lists, label/reltype indexes, node vectors)
- **Node.js GraphIndex bindings**: `cypher()`, `createNode()`, `createEdge()`, `nodesByLabel()`, etc.
- **CLI graph commands**: `vectrust graph stats`, `vectrust graph query`, `vectrust graph create`
- **New crates**: `vectrust-cypher` (parser), `vectrust-graph` (storage + executor)
- Graph example (`examples/graph.rs`)
- 19 integration tests for graph functionality
- 113+ total tests across workspace

### Changed
- `GraphIndex::open()` now creates a shared RocksDB with all column families
- README updated with graph + Cypher documentation
- CLI renamed from `vectra` to `vectrust`

### Backward Compatible
- `LocalIndex` API unchanged — zero breaking changes for existing vector-only users
- Existing vector storage format fully compatible

## [Unreleased]

### Added
- Initial Rust implementation of vector database
- HNSW (Hierarchical Navigable Small World) indexing algorithm
- Optimized v2 storage format with RocksDB backend
- Node.js bindings via NAPI
- Comprehensive benchmark suite
- Multiple similarity metrics (Cosine, Euclidean, Dot Product)
- Transaction support with commit/rollback
- JSON metadata support with filtering
- Command-line interface (CLI)
- Memory-mapped vector storage for performance
- Batch insert operations for improved throughput

### Changed
- Migrated from Node.js/TypeScript to Rust for core implementation
- Renamed project from "vectra" to "vectrust"
- Improved storage performance by 13-73% over legacy format
- Optimized RocksDB configuration for vector workloads
- Implemented batched manifest updates to reduce I/O

### Performance
- **Single Insert**: 0.246ms average (4,000+ ops/sec)
- **Bulk Insert**: 0.065ms per item (15,000+ items/sec) 
- **Search Query**: 0.742ms average (1,300+ queries/sec)
- **Index Creation**: 0.319ms (instant)
- **Index Loading**: 0.003ms (instant)

### Technical Details
- Rust 1.70+ compatibility
- Multi-crate workspace architecture
- Async/await throughout with Tokio runtime
- Memory-mapped file I/O for vector data
- RocksDB for metadata and indexing
- Comprehensive test coverage
- Release-mode optimizations

## [0.1.0] - 2025-01-29

### Added
- Initial release of Vectrust
- Core vector database functionality
- High-performance storage and indexing
- Node.js bindings for JavaScript integration
- Complete documentation and examples