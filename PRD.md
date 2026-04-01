# Vectrust — Product Requirements Document

> The first embeddable graph + vector database with Cypher query language support.

## Vision

AI applications need both **relationships** (who calls what, what depends on what) and **similarity** (what code is similar, what documents are related). Today this requires stitching together separate systems — Neo4j for graphs, Pinecone/Qdrant for vectors, or building custom layers on RocksDB.

Vectrust eliminates this by providing graph traversal and vector similarity in a single embeddable Rust library with a Cypher query interface. No server process, no JVM, no network overhead. One dependency, one data directory, one query language.

## Current State (v0.1.4)

- 7 crates, ~3400 LOC
- Vector storage with RocksDB backend (optimized v2 format)
- HNSW indexing via `instant-distance`
- Node.js bindings (NAPI)
- ACID transactions (basic)
- Cosine, Euclidean, Dot Product similarity
- Benchmarks: sub-ms search, 15K+ bulk inserts/sec

## Target State (v0.2.0)

Graph-capable vector database with Cypher subset + vector extensions. Backward-compatible — existing vector-only API unchanged.

## Target Users

1. **MCP server builders** — building code intelligence, knowledge graphs, or agent memory tools in Rust
2. **RAG pipeline developers** — need knowledge graph + semantic search without running Neo4j + Pinecone
3. **Local-first AI tool builders** — VS Code extensions, CLI tools, desktop apps that need structured data + embeddings
4. **Node.js/TypeScript developers** — full-stack apps needing embedded graph+vector via native bindings

## Competitive Landscape

| Database | Graph | Vectors | Embeddable | Footprint | Query Language |
|----------|-------|---------|------------|-----------|---------------|
| Neo4j | Yes | Plugin | No (server) | 500MB+ | Cypher |
| SQLite | No | No* | Yes | 1MB | SQL |
| RocksDB | No (KV) | No | Yes | 10MB | None |
| LanceDB | No | Yes | Yes | 15MB | SQL-like |
| DuckDB | No | No | Yes | 20MB | SQL |
| SurrealDB | Partial | Partial | Partial | 30MB | SurrealQL |
| **Vectrust** | **Yes** | **Yes** | **Yes** | **<5MB** | **Cypher subset** |

Nobody is Graph + Vector + Embeddable + Small + Cypher. That's the gap.

## Query Language: Cypher Subset + Vector Extensions

### Why Cypher
- Industry standard for graph queries (millions of developers know it)
- Natural fit for relationship traversal (unlike SQL)
- ISO GQL standard is based on Cypher
- Parser is tractable (~2000 LOC for a useful subset)

### Phase 1 — MVP Cypher

**Data Manipulation:**
```cypher
CREATE (n:Label {key: value, ...})
CREATE (a)-[:REL_TYPE {props}]->(b)
SET n.property = value
REMOVE n.property
DELETE n
DETACH DELETE n
```

**Reading:**
```cypher
MATCH (n:Label)
MATCH (a)-[:REL]->(b)
MATCH (a)-[:REL*1..3]->(b)      -- variable-length paths
WHERE n.prop = value              -- =, <>, <, >, <=, >=, AND, OR, NOT, IN, CONTAINS, STARTS WITH
RETURN n, n.prop, count(*)
ORDER BY n.prop DESC
LIMIT 10
SKIP 5
WITH                              -- query chaining
```

**Vector Extensions (the differentiator):**
```cypher
-- Similarity as a function (works in WHERE, RETURN, ORDER BY)
MATCH (n:Document)
WHERE vector_similarity(n.embedding, $query) > 0.8
RETURN n.title, vector_similarity(n.embedding, $query) AS score
ORDER BY score DESC

-- Distance function
MATCH (n:Function)
RETURN n, vector_distance(n.embedding, $query) AS dist
ORDER BY dist ASC LIMIT 10

-- HNSW-accelerated kNN (procedure call syntax)
CALL vectrust.nearest('embedding', $query, 10) YIELD node, score
RETURN node.name, score
```

### Phase 2 — Deferred
- MERGE (upsert)
- OPTIONAL MATCH
- UNWIND, FOREACH
- Full aggregations (sum, avg, min, max, collect)
- CASE WHEN
- UNION
- Subqueries, EXISTS
- SHORTEST PATH

## Architecture

### Crate Structure (10 crates)

```
crates/
  vectrust-core/        # Core types: Node, Edge, Vector, Properties, Errors
  vectrust-storage/     # RocksDB backend: vector CFs + graph CFs
  vectrust-index/       # HNSW, flat index, quantized index
  vectrust-cypher/      # NEW: Cypher lexer + parser + AST
  vectrust-graph/       # NEW: Graph storage, traversal, planner, executor
  vectrust-query/       # Query execution engine (repurposed from stubs)
  vectrust/             # Facade: LocalIndex (vector) + GraphIndex (graph+vector)
  vectrust-cli/         # CLI tool
  vectrust-node/        # Node.js NAPI bindings
```

### Parser: logos + Recursive Descent

- **Lexer**: `logos` crate — fast, derive-based tokenizer
- **Parser**: Hand-written recursive descent — best error messages, simple to extend
- **AST**: Strongly typed Rust enums — `Statement`, `Clause`, `Pattern`, `Expression`
- **No runtime dependencies** — parser is a pure function from string to AST

Why not pest/nom/tree-sitter:
- pest: poor error recovery, PEG fights left-recursive rules
- nom: verbose for language parsing, impenetrable type errors
- tree-sitter: overkill (designed for incremental editor parsing), adds C dependency

### Graph Storage: RocksDB Column Families

Extend existing RocksDB instance with new CFs:

| CF Name | Key Format | Value | Purpose |
|---------|-----------|-------|---------|
| `graph_nodes` | `n:{uuid}` | bincode NodeRecord | Node existence + labels |
| `graph_node_props` | `np:{uuid}` | JSON properties | Node properties |
| `graph_edges` | `e:{uuid}` | bincode EdgeRecord | Edge source, target, type |
| `graph_edge_props` | `ep:{uuid}` | JSON properties | Edge properties |
| `graph_adj_out` | `ao:{src}:{edge}` | target uuid | Outgoing adjacency |
| `graph_adj_in` | `ai:{tgt}:{edge}` | source uuid | Incoming adjacency |
| `graph_label_idx` | `li:{label}:{node}` | empty | Label index |
| `graph_reltype_idx` | `ri:{type}:{edge}` | empty | Relationship type index |

Key design decisions:
- **Properties separate from records** — traversal (hot path) doesn't deserialize properties
- **Adjacency via prefix scans** — RocksDB sorted keys enable efficient neighbor lookup
- **Nodes own vectors** — `NodeRecord.has_vector` flag bridges to existing vector storage
- **Shared RocksDB instance** — single data directory, single write lock

### Query Execution: Volcano Model

```
Cypher string → [Parser] → AST → [Planner] → LogicalPlan → [Optimizer] → PhysicalPlan → [Executor] → Iterator<Row>
```

Logical operators:
- `NodeScan` — scan nodes by label
- `Expand` — traverse edges (single-hop or variable-length)
- `VectorKnn` — HNSW-accelerated nearest neighbors
- `Filter` — WHERE predicate evaluation
- `Project` — RETURN expression evaluation
- `Sort`, `Limit`, `Skip` — standard relational operators
- `CreateNode`, `CreateEdge`, `DeleteNodes`, `SetProperty` — mutations

Combined query example:
```cypher
MATCH (doc:Document)-[:REFERENCES]->(ref:Document)
WHERE doc.topic = 'AI'
RETURN ref, vector_similarity(ref.embedding, $query) AS sim
ORDER BY sim DESC LIMIT 5
```
Plan: `Limit → Sort → Project → Expand → Filter → NodeScan`

## API Design

### Rust

```rust
// Existing vector API (unchanged)
let db = vectrust::LocalIndex::new("./data", None)?;
db.insert_item(item).await?;
let results = db.query_items(vec, Some(10), None).await?;

// New graph API
let db = vectrust::GraphIndex::new("./data")?;

// Programmatic
let alice = db.create_node(&["Person"], json!({"name": "Alice"})).await?;
let bob = db.create_node(&["Person"], json!({"name": "Bob"})).await?;
db.create_edge(alice, bob, "KNOWS", json!({"since": 2020})).await?;

// Cypher
let results = db.cypher("MATCH (p:Person)-[:KNOWS]->(f) RETURN f.name").await?;

// Cypher with parameters
let results = db.cypher_with_params(
    "MATCH (n:Doc) WHERE vector_similarity(n.embedding, $q) > $t RETURN n",
    json!({"q": [0.1, 0.2, ...], "t": 0.8})
).await?;

// Vector-aware node
let doc = db.create_node_with_vector(
    &["Document"],
    json!({"title": "Paper"}),
    vec![0.1, 0.2, 0.3, ...],
).await?;
```

### Node.js

```typescript
import { GraphIndex } from 'vectrust';

const db = new GraphIndex('./data');
await db.createGraph();

const results = await db.cypher(
  'MATCH (p:Person)-[:KNOWS]->(f) WHERE p.name = $name RETURN f',
  { name: 'Alice' }
);

// Backward-compatible vector API still works
const vectorDb = new LocalIndex('./data');
```

### Return Types

```rust
enum GraphValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<GraphValue>),
    Map(HashMap<String, GraphValue>),
    Node(GraphNode),
    Edge(GraphEdge),
    Path(GraphPath),
}
```

## Migration & Backward Compatibility

- `LocalIndex` API unchanged — zero breaking changes
- `GraphIndex` opens same database, creates graph CFs on first use
- Existing `VectorItem` accessible as graph nodes lazily
- CLI: `vectrust migrate --to-graph ./data` for explicit conversion
- Manifest v3 adds `graph_enabled`, node/edge counts, label/reltype lists

## Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Node creation | < 50us | Sync to memory, async to disk |
| Edge creation | < 100us | Includes adjacency list update |
| Batch node creation | > 50K/sec | |
| Single-hop expansion | < 10us/edge | Adjacency prefix scan |
| 1-hop neighborhood | < 1ms | Degree < 1000 |
| 3-hop BFS | < 10ms | Graphs < 100K edges |
| Variable-length path (1..5) | < 50ms | Bounded result sets |
| Graph filter + vector sort | < 50ms | Result sets < 10K |
| Vector kNN + 1-hop expand | < 20ms | k=100 |
| Memory overhead | < 10MB | Base, excluding data |
| Binary contribution | < 5MB | Statically linked |

## Implementation Plan

### Week 1-2: Parser + Core Types
- logos lexer with all Cypher tokens
- Recursive descent parser for MVP subset
- AST node types
- Graph types in vectrust-core (GraphNode, GraphEdge, GraphValue)
- Comprehensive parser tests

### Week 3: Graph Storage
- Graph column families in OptimizedStorage
- Node/edge CRUD operations
- Adjacency list prefix scans
- Label and relationship type indexes
- Storage unit tests

### Week 4: Query Execution
- AST → logical plan translation
- Physical operators: NodeScan, Expand, Filter, Project, Sort, Limit
- Expression evaluator (property access, comparisons, boolean ops)
- vector_distance() / vector_similarity() functions
- Integration tests: parse → plan → execute → results

### Week 5: Integration + API
- GraphIndex facade
- Cypher string API with parameter binding
- Backward compatibility verification
- Node.js binding updates
- End-to-end tests

### Week 6: Polish + Release
- Error messages and diagnostics
- Documentation and examples
- Benchmarks (vs Neo4j embedded, vs raw RocksDB)
- CLI updates (graph stats, node/edge counts)
- v0.2.0 release

## First Customer: CodeGraph

Replace CodeGraph's custom graph implementation (`codegraph` crate with HashMap-based graph + RocksDB persistence + custom HNSW) with Vectrust. This validates the API on a real production tool with 35 MCP tools, 17 language parsers, and real-world query patterns.

Migration: `codegraph::CodeGraph` → `vectrust::GraphIndex` with Cypher queries replacing imperative graph traversal code.

## Success Metrics

- v0.2.0 shipped with working Cypher subset
- CodeGraph migrated to Vectrust as storage backend
- npm package downloads > 100/week
- crates.io downloads > 50/week
- At least one external project using Vectrust for graph+vector
- Sub-ms query latency for typical AI application patterns
- < 5MB binary contribution (embedded use case)

## Non-Goals (for now)

- Full Cypher compliance (subset is enough)
- Distributed/clustered operation
- HTTP/Bolt server mode
- Schema enforcement
- Full-text search index
- Time-series support
- Replication
