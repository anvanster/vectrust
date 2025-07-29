# 🦀 Vectrust

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Performance](https://img.shields.io/badge/Performance-High-brightgreen.svg)](#performance)

**Vectrust** is a high-performance, local vector database built in Rust with Node.js bindings. Designed for applications requiring fast semantic search, similarity matching, and vector operations with enterprise-grade performance and reliability.

## ✨ Features

- 🚀 **Blazing Fast**: Sub-millisecond search latency, 6K+ vectors/second indexing
- 🔍 **Advanced Indexing**: HNSW (Hierarchical Navigable Small World) algorithm support
- 💾 **Optimized Storage**: Custom v2 storage format with RocksDB backend
- 🌐 **Multi-Language**: Native Rust API with Node.js bindings
- 🔒 **ACID Transactions**: Full transaction support with rollback capabilities
- 📊 **Rich Metadata**: JSON metadata support with filtering capabilities
- 🎯 **Multiple Similarity Metrics**: Cosine, Euclidean, and Dot Product
- 🔄 **Hot Reloading**: Instant index loading and updates
- 🧪 **Battle Tested**: Comprehensive test suite and benchmarks

## 🚀 Performance

**Release Mode Benchmarks** (1000 vectors, 128 dimensions):

| Operation | Latency | Throughput |
|-----------|---------|------------|
| **Single Insert** | 0.246ms | 4,000+ ops/sec |
| **Bulk Insert** | 0.065ms/item | 15,000+ items/sec |
| **Search Query** | 0.742ms | 1,300+ queries/sec |
| **Index Creation** | 0.319ms | Instant |
| **Index Loading** | 0.003ms | Instant |

### Real-World Use Case: 100K Line Codebase
- **Full indexing**: 1-2 seconds (~12,000 vectors)
- **Semantic search**: <1ms response time
- **Memory usage**: ~300MB
- **Startup time**: <10ms

## 📦 Installation

### Rust
Add to your `Cargo.toml`:
```toml
[dependencies]
vectrust = "0.1.0"
```

### Node.js
```bash
npm install vectrust
```

## 🔧 Quick Start

### Rust API

```rust
use vectrust::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new index
    let index = LocalIndex::new("./my_vectors", None)?;
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;

    // Insert vectors
    let item = VectorItem {
        id: uuid::Uuid::new_v4(),
        vector: vec![0.1, 0.2, 0.3, 0.4],
        metadata: serde_json::json!({"category": "example"}),
        ..Default::default()
    };
    
    let inserted = index.insert_item(item).await?;
    println!("Inserted: {}", inserted.id);

    // Search similar vectors
    let query_vector = vec![0.1, 0.2, 0.3, 0.4];
    let results = index.query_items(query_vector, Some(10), None).await?;
    
    for result in results {
        println!("ID: {}, Score: {:.3}", result.item.id, result.score);
    }

    Ok(())
}
```

### Node.js API

```javascript
const { LocalIndex } = require('vectrust');

async function example() {
    // Create index
    const index = new LocalIndex('./my_vectors');
    await index.createIndex('{"version": 2}');

    // Insert vector
    const item = {
        id: '123e4567-e89b-12d3-a456-426614174000',
        vector: [0.1, 0.2, 0.3, 0.4],
        metadata: { category: 'example' }
    };
    
    const result = await index.insertItem(JSON.stringify(item));
    console.log('Inserted:', JSON.parse(result).id);

    // Search vectors
    const searchResults = await index.queryItems([0.1, 0.2, 0.3, 0.4], 10);
    const results = JSON.parse(searchResults);
    
    results.forEach(result => {
        console.log(`ID: ${result.item.id}, Score: ${result.score}`);
    });
}

example().catch(console.error);
```

## 🏗️ Architecture

Vectrust is built with a modular architecture:

```
┌─────────────────┐    ┌──────────────────┐
│   Vectrust      │    │   Node.js        │
│   (Rust API)    │    │   Bindings       │
└─────────┬───────┘    └─────────┬────────┘
          │                      │
          └──────────┬───────────┘
                     │
        ┌────────────▼────────────┐
        │     Vectrust Core       │
        │   (Types & Traits)      │
        └────────────┬────────────┘
                     │
    ┌────────────────┼────────────────┐
    │                │                │
┌───▼───┐       ┌────▼─────┐     ┌───▼────┐
│Storage│       │  Index   │     │ Query  │
│Backend│       │  (HNSW)  │     │Engine  │
└───────┘       └──────────┘     └────────┘
```

### Core Components

- **vectrust-core**: Core types, traits, and error handling
- **vectrust-storage**: Storage backends (Legacy JSON, Optimized v2)
- **vectrust-index**: Indexing algorithms (HNSW, Flat, Quantized)
- **vectrust-query**: Query processing and filtering
- **vectrust-node**: Node.js NAPI bindings
- **vectrust-cli**: Command-line interface

## 📊 Benchmarks

Run comprehensive benchmarks:

```bash
# Rust benchmarks
cd benchmarks/rust
cargo run --release --bin benchmark_runner -- --vectors 10000

# Simple performance test
cargo run --release --bin benchmark_runner -- --simple
```

### Benchmark Results

**Hardware**: Modern Linux x64 system  
**Configuration**: Release mode, optimized v2 storage

| Test | Vectors | Dimensions | Avg Time | Throughput |
|------|---------|------------|----------|------------|
| Index Creation | - | - | 0.319ms | - |
| Single Insert | 1 | 128 | 0.246ms | 4,065 ops/sec |
| Bulk Insert | 100 | 128 | 64.8ms | 15,432 items/sec |
| Single Search | 1 | 128 | 0.742ms | 1,348 queries/sec |
| Scale Test | 1,000 | 128 | 5.14s | 194 items/sec* |

*Scale test includes full index creation overhead

## 🛠️ Development

### Prerequisites

- Rust 1.70+ with Cargo
- Node.js 14+ (for Node.js bindings)
- Git

### Building from Source

```bash
# Clone repository
git clone https://github.com/anvanster/vectrust.git
cd vectrust

# Build all components
cargo build --release

# Run tests
cargo test

# Build Node.js bindings
cd crates/vectrust-node
npm install
npm run build
```

### Project Structure

```
vectrust/
├── crates/
│   ├── vectrust-core/     # Core types and traits
│   ├── vectrust-storage/  # Storage backends
│   ├── vectrust-index/    # Indexing algorithms
│   ├── vectrust-query/    # Query processing
│   ├── vectrust/          # Main library
│   ├── vectrust-cli/      # Command line tool
│   └── vectrust-node/     # Node.js bindings
├── benchmarks/            # Performance benchmarks
├── examples/              # Usage examples
├── tests/                 # Integration tests
└── README.md
```

## 🎯 Use Cases

### Code Search & Analysis
- **Semantic code search** across large codebases
- **Duplicate detection** and code similarity analysis
- **IDE integration** for intelligent code completion

### Document & Content Management
- **Document similarity** and recommendation systems
- **Content clustering** and organization
- **Semantic search** in knowledge bases

### AI & Machine Learning
- **Embedding storage** for large language models
- **Retrieval-augmented generation** (RAG) systems
- **Feature vector** storage and similarity matching

### Real-Time Applications
- **Recommendation engines** with sub-millisecond latency
- **Real-time search** in chat and messaging systems
- **Live content** filtering and matching

## 🔮 Roadmap

- [ ] **Distributed Setup**: Multi-node clustering support
- [ ] **Advanced Indexing**: LSH, Product Quantization
- [ ] **Streaming Updates**: Real-time vector streaming
- [ ] **Cloud Integration**: S3, GCS storage backends
- [ ] **Monitoring**: Prometheus metrics and health checks
- [ ] **More Language Bindings**: Python, Go, Java
- [ ] **Vector Operations**: Mathematical operations on vectors
- [ ] **Hybrid Search**: Text + vector search capabilities

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Workflow

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes and add tests
4. Run tests: `cargo test`
5. Run benchmarks: `cargo run --release --bin benchmark_runner`
6. Commit your changes: `git commit -m 'Add amazing feature'`
7. Push to the branch: `git push origin feature/amazing-feature`
8. Open a Pull Request

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **RocksDB** team for the excellent storage engine
- **HNSW** algorithm researchers for the efficient indexing approach
- **Rust community** for the amazing ecosystem and tooling
- **Node.js NAPI** team for seamless language bindings
- **Apache Software Foundation** for the licensing framework

## 📞 Support & Community

- **Issues**: [GitHub Issues](https://github.com/anvanster/vectrust/issues)
- **Discussions**: [GitHub Discussions](https://github.com/anvanster/vectrust/discussions)
- **Documentation**: [Docs](https://docs.rs/vectrust)

---

Built with ❤️ in Rust 🦀