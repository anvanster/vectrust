# Changelog

All notable changes to Vectrust will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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