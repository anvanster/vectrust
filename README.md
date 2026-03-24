# Vectrust

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

**Vectrust** is the first embeddable graph + vector database with Cypher query language support. Graph traversal and vector similarity in a single Rust library. No server process, no JVM, no network overhead.

## Features

- **Graph + Vector**: Nodes, edges, properties, and embedding vectors in one database
- **Cypher Queries**: Industry-standard graph query language with vector extensions
- **Embeddable**: Single library, single data directory, <5MB binary contribution
- **Vector Search**: Cosine similarity, Euclidean distance, Dot Product with kNN support
- **HNSW Indexing**: Fast approximate nearest neighbor search
- **RocksDB Backend**: Optimized storage with column families for graph and vector data
- **Node.js Bindings**: Full API available via native NAPI bindings
- **ACID Transactions**: Transaction support with rollback capabilities

## Quick Start

### Graph + Vector (Rust)

```rust
use vectrust::GraphIndex;

fn main() -> vectrust::Result<()> {
    let db = GraphIndex::open("./data")?;

    // Create nodes
    let alice = db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))?;
    let bob = db.create_node(&["Person"], serde_json::json!({"name": "Bob"}))?;
    db.create_edge(alice.id, bob.id, "KNOWS", serde_json::json!({"since": 2020}))?;

    // Query with Cypher
    let result = db.cypher("MATCH (p:Person)-[:KNOWS]->(f) RETURN f.name AS friend")?;
    for row in &result.rows {
        println!("Friend: {:?}", row.get("friend"));
    }

    // Nodes with vectors
    let doc = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "AI Paper"}),
        vec![0.1, 0.2, 0.3],
    )?;

    // Combined graph + vector query
    let result = db.cypher_with_params(
        "MATCH (n:Document) \
         WHERE vector_similarity(n.embedding, $query) > 0.8 \
         RETURN n.title, vector_similarity(n.embedding, $query) AS score \
         ORDER BY score DESC",
        serde_json::json!({"query": [0.1, 0.2, 0.3]}),
    )?;

    // kNN search + graph traversal
    let result = db.cypher_with_params(
        "CALL vectrust.nearest('embedding', $q, 10) YIELD node, score \
         MATCH (author:Person)-[:AUTHORED]->(node) \
         RETURN author.name, node.title, score",
        serde_json::json!({"q": [0.1, 0.2, 0.3]}),
    )?;

    Ok(())
}
```

### Vector-Only (Rust)

The existing vector API is unchanged and fully backward-compatible:

```rust
let index = vectrust::LocalIndex::new("./vectors", None)?;
index.create_index(None).await?;
index.insert_item(item).await?;
let results = index.query_items(vec![0.1, 0.2, 0.3], Some(10), None).await?;
```

### Node.js

```javascript
const { GraphIndex, LocalIndex } = require('vectrust');

// Graph + Vector
const db = new GraphIndex('./data');
db.cypher("CREATE (n:Person {name: 'Alice', age: 30})");
const result = db.cypher("MATCH (n:Person) WHERE n.age > 25 RETURN n.name");
console.log(JSON.parse(result));

// Vector-only (backward compatible)
const vectorDb = new LocalIndex('./vectors');
```

### CLI

```bash
# Execute Cypher queries
vectrust graph query --path ./data "MATCH (n:Person) RETURN n.name"

# Graph statistics
vectrust graph stats --path ./data

# Vector index statistics
vectrust stats --path ./vectors
```

## Cypher Support

### Data Manipulation
```cypher
CREATE (n:Label {key: value, ...})
CREATE (a)-[:REL_TYPE {props}]->(b)
SET n.property = value
DELETE n
DETACH DELETE n
```

### Reading
```cypher
MATCH (n:Label)
MATCH (a)-[:REL]->(b)
MATCH (a)-[:REL*1..3]->(b)          -- variable-length paths
WHERE n.prop = value                  -- =, <>, <, >, <=, >=, AND, OR, NOT
RETURN n, n.prop, count(*), collect(n.name)
ORDER BY n.prop DESC
LIMIT 10
SKIP 5
WITH                                  -- query chaining
```

### Vector Extensions
```cypher
-- Similarity filtering and ranking
MATCH (n:Document)
WHERE vector_similarity(n.embedding, $query) > 0.8
RETURN n.title, vector_similarity(n.embedding, $query) AS score
ORDER BY score DESC

-- kNN search (procedure call syntax)
CALL vectrust.nearest('embedding', $query, 10) YIELD node, score
RETURN node.name, score

-- kNN + graph traversal (the differentiator)
CALL vectrust.nearest('embedding', $query, 10) YIELD node, score
MATCH (author:Person)-[:AUTHORED]->(node)
RETURN author.name, node.title, score
```

## Architecture

```
                    ┌──────────────────────────────────────┐
                    │           User Application           │
                    └──────────┬──────────────┬────────────┘
                               │              │
                    ┌──────────▼──────┐  ┌────▼─────────┐
                    │   GraphIndex    │  │  LocalIndex   │
                    │  (Graph+Vector) │  │ (Vector-only) │
                    └────────┬────────┘  └──────┬────────┘
                             │                  │
          ┌──────────────────┼──────────────────┘
          │                  │
┌─────────▼────────┐  ┌─────▼──────────┐  ┌──────────────┐
│ vectrust-cypher   │  │ vectrust-graph  │  │vectrust-index│
│ Lexer + Parser    │  │ Storage+Exec   │  │   HNSW       │
└──────────────────┘  └───────┬────────┘  └──────────────┘
                              │
                    ┌─────────▼─────────┐
                    │    RocksDB        │
                    │  9 Column Families│
                    └───────────────────┘
```

### Crates

| Crate | Purpose |
|-------|---------|
| `vectrust-core` | Core types: GraphNode, GraphEdge, GraphValue, VectorItem, errors |
| `vectrust-cypher` | Cypher lexer (logos) + recursive descent parser + AST |
| `vectrust-graph` | Graph storage (RocksDB CFs), query planner, executor |
| `vectrust-storage` | Vector storage backend (RocksDB + mmap) |
| `vectrust-index` | HNSW, flat, and quantized indexes |
| `vectrust` | Facade: GraphIndex + LocalIndex |
| `vectrust-cli` | CLI tool |
| `vectrust-node` | Node.js NAPI bindings |

## Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Node creation | <1ms | Including index updates |
| Edge creation | <1ms | Including adjacency lists |
| Single-hop traversal | <1ms | Prefix scan |
| Cypher query (simple) | <5ms | Parse + execute |
| Vector kNN (k=10) | <1ms | Brute-force on small sets |
| Single vector insert | 0.246ms | Optimized v2 format |
| Bulk vector insert | 0.065ms/item | 15K+ items/sec |
| Vector search | 0.742ms | Cosine similarity |

## Installation

### Rust
```toml
[dependencies]
vectrust = "0.1.4"
```

### Node.js
```bash
npm install vectrust
```

## Development

```bash
cargo build --release    # Build
cargo test               # Run all tests (100+)
cargo run --bin graph    # Run graph example
```

### Project Structure

```
vectrust/
├── crates/
│   ├── vectrust-core/      # Core types and traits
│   ├── vectrust-cypher/     # Cypher parser
│   ├── vectrust-graph/      # Graph storage + executor
│   ├── vectrust-storage/    # Vector storage backends
│   ├── vectrust-index/      # Indexing algorithms
│   ├── vectrust-query/      # Query processing
│   ├── vectrust/            # Facade library
│   ├── vectrust-cli/        # CLI tool
│   └── vectrust-node/       # Node.js bindings
├── examples/                # Usage examples
├── tests/                   # Integration tests
└── benchmarks/              # Performance benchmarks
```

## License

Apache License 2.0 - see [LICENSE](LICENSE).
