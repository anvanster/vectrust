# CodeGraph → Vectrust Migration Guide

This guide maps CodeGraph's existing API to Vectrust equivalents. Vectrust replaces three separate data stores (in-memory HashMap graph, RocksDB KV, custom HNSW) with a single `GraphIndex`.

## Setup

```rust
// Before: three separate stores
let graph = codegraph::CodeGraph::new();
let db = rocksdb::DB::open(&opts, path)?;
let hnsw = HnswIndex::new(config);

// After: one unified store
let db = vectrust::GraphIndex::open("./data")?;
```

## Node Operations

### Creating Nodes

```rust
// Before
let node_id = graph.add_node("Function", props);

// After — programmatic
let node = db.create_node(&["Function"], serde_json::json!({
    "name": "main",
    "file": "src/main.rs",
    "complexity": 5,
}))?;
let node_id = node.id;

// After — Cypher
db.cypher("CREATE (n:Function {name: 'main', file: 'src/main.rs', complexity: 5})")?;

// After — with embedding vector
let node = db.create_node_with_vector(
    &["Function"],
    serde_json::json!({"name": "main"}),
    embedding_vec,  // Vec<f32>, e.g. 768d from Jina Code V2
)?;
```

### Multi-label Nodes (visibility as secondary label)

```rust
// Before
let node_id = graph.add_node("Function", props);
graph.set_property(node_id, "visibility", "public");

// After — labels replace property-based filtering
let node = db.create_node(&["Function", "Public"], serde_json::json!({
    "name": "main",
}))?;

// Query: find all public functions
db.cypher("MATCH (n:Function:Public) RETURN n.name")?;
```

### Bulk Node Creation

```rust
// Before
for func in parsed_functions {
    graph.add_node("Function", func.props);
}

// After — batch for speed (84K nodes/sec)
let batch: Vec<(&[&str], serde_json::Value)> = parsed_functions.iter()
    .map(|f| (&["Function"][..], serde_json::json!({"name": f.name, "file": f.file})))
    .collect();
let ids = db.create_nodes_batch(&batch)?;

// After — batch Cypher
let stmts: Vec<String> = parsed_functions.iter()
    .map(|f| format!("CREATE (n:Function {{name: '{}', file: '{}'}})", f.name, f.file))
    .collect();
let refs: Vec<&str> = stmts.iter().map(|s| s.as_str()).collect();
db.cypher_batch(&refs)?;
```

## Edge Operations

### Creating Edges

```rust
// Before
graph.add_edge(caller_id, callee_id, "CALLS");

// After — programmatic
db.create_edge(caller_id, callee_id, "CALLS", serde_json::json!({}))?;

// After — Cypher (when you have node properties, not IDs)
db.cypher(
    "MATCH (a:Function), (b:Function) \
     WHERE a.name = 'main' AND b.name = 'helper' \
     CREATE (a)-[:CALLS]->(b)"
)?;
```

### Edge Types Used in CodeGraph

| CodeGraph edge | Vectrust relationship type |
|---------------|---------------------------|
| `graph.add_edge(a, b, "calls")` | `CREATE (a)-[:CALLS]->(b)` |
| `graph.add_edge(a, b, "imports")` | `CREATE (a)-[:IMPORTS]->(b)` |
| `graph.add_edge(a, b, "contains")` | `CREATE (a)-[:CONTAINS]->(b)` |
| `graph.add_edge(a, b, "implements")` | `CREATE (a)-[:IMPLEMENTS]->(b)` |
| `graph.add_edge(a, b, "references")` | `CREATE (a)-[:REFERENCES]->(b)` |

## Querying

### Property Filtering

```rust
// Before
let results: Vec<_> = graph.iter_nodes()
    .filter(|n| n.label == "Function" && n.props["complexity"] > 10)
    .collect();

// After
let result = db.cypher(
    "MATCH (n:Function) WHERE n.complexity > 10 RETURN n.name, n.complexity ORDER BY n.complexity DESC"
)?;
for row in &result.rows {
    println!("{}: {}", row["n.name"], row["n.complexity"]);
}
```

### Edge Traversal

```rust
// Before
let edges = graph.get_edges_from(node_id, "calls");
let callees: Vec<_> = edges.iter().map(|e| graph.get_node(e.target)).collect();

// After
let result = db.cypher_with_params(
    "MATCH (a:Function)-[:CALLS]->(b:Function) WHERE a.name = $name RETURN b.name, b.file",
    serde_json::json!({"name": "main"}),
)?;
```

### Multi-hop Traversal

```rust
// Before (imperative BFS)
fn find_transitive_deps(graph: &CodeGraph, start: NodeId, depth: usize) -> Vec<NodeId> {
    let mut visited = HashSet::new();
    let mut frontier = vec![start];
    for _ in 0..depth {
        // ... manual BFS
    }
    visited.into_iter().collect()
}

// After — one line of Cypher
let result = db.cypher_with_params(
    "MATCH (a:Function)-[:CALLS*1..3]->(b:Function) WHERE a.name = $name RETURN DISTINCT b.name",
    serde_json::json!({"name": "main"}),
)?;
```

### Cross-File Import Resolution

```rust
// Before
fn resolve_cross_file_imports(graph: &CodeGraph) -> Vec<(String, String)> {
    let mut results = vec![];
    for node in graph.iter_nodes_by_label("Import") {
        for edge in graph.get_edges_from(node.id, "resolves_to") {
            let target = graph.get_node(edge.target);
            if target.props["file"] != node.props["file"] {
                results.push((node.props["file"].clone(), target.props["file"].clone()));
            }
        }
    }
    results
}

// After
let result = db.cypher(
    "MATCH (i:Import)-[:RESOLVES_TO]->(t:Function) \
     WHERE i.file <> t.file \
     RETURN i.file AS from_file, t.file AS to_file"
)?;
```

## Vector Search

### Similarity Search

```rust
// Before
let query_vec = embed("function that handles authentication");
let mut scores: Vec<_> = all_vectors.iter()
    .map(|(id, vec)| (id, cosine_similarity(&query_vec, vec)))
    .collect();
scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
let top_10 = &scores[..10];

// After — Cypher with vector_similarity()
let result = db.cypher_with_params(
    "MATCH (n:Function) \
     WHERE vector_similarity(n.embedding, $query) > 0.7 \
     RETURN n.name, vector_similarity(n.embedding, $query) AS score \
     ORDER BY score DESC LIMIT 10",
    serde_json::json!({"query": query_vec}),
)?;

// After — CALL vectrust.nearest (HNSW-accelerated, <1ms)
let result = db.cypher_with_params(
    "CALL vectrust.nearest('embedding', $query, 10) YIELD node, score \
     RETURN node.name, score",
    serde_json::json!({"query": query_vec}),
)?;
```

### Combined: Similar Functions + Their Callers

```rust
// Before: two separate queries stitched together in Rust
let similar = hnsw.search(&query_vec, 10);
for (id, score) in similar {
    let callers = graph.get_edges_to(id, "calls");
    for caller_edge in callers {
        let caller = graph.get_node(caller_edge.source);
        println!("{} calls {} (score: {})", caller.name, graph.get_node(id).name, score);
    }
}

// After: one Cypher query
let result = db.cypher_with_params(
    "CALL vectrust.nearest('embedding', $query, 10) YIELD node, score \
     MATCH (caller:Function)-[:CALLS]->(node) \
     RETURN caller.name AS caller, node.name AS callee, score \
     ORDER BY score DESC",
    serde_json::json!({"query": query_vec}),
)?;
```

## Incremental Re-indexing (MERGE)

```rust
// Before
if let Some(existing) = graph.find_node_by_prop("Function", "name", &func.name) {
    graph.update_properties(existing, new_props);
} else {
    graph.add_node("Function", new_props);
}

// After — MERGE handles this atomically
db.cypher_with_params(
    "MERGE (n:Function {name: $name}) \
     ON CREATE SET n.file = $file, n.complexity = $complexity \
     ON MATCH SET n.complexity = $complexity",
    serde_json::json!({
        "name": func.name,
        "file": func.file,
        "complexity": func.complexity,
    }),
)?;
```

## Aggregation

```rust
// Before
let mut counts: HashMap<String, usize> = HashMap::new();
for node in graph.iter_nodes_by_label("Function") {
    *counts.entry(node.props["file"].clone()).or_default() += 1;
}

// After
let result = db.cypher(
    "MATCH (n:Function) \
     RETURN n.file AS file, count(*) AS functions, avg(n.complexity) AS avg_complexity \
     ORDER BY functions DESC"
)?;
```

## Property Indexes

For fast lookups on frequently-queried properties, create indexes:

```rust
// Create once at startup
db.create_property_index("Function", "name")?;
db.create_property_index("Function", "file")?;

// Now MATCH and MERGE on these properties use O(1) lookup instead of full scan
```

## Import/Export

```rust
// Export existing graph to JSON (for backup or migration)
let data = db.export_json()?;
std::fs::write("graph.json", serde_json::to_string_pretty(&data)?)?;

// Import from JSON
let json = std::fs::read_to_string("graph.json")?;
let data: vectrust::GraphJson = serde_json::from_str(&json)?;
let (nodes, edges) = db.import_json(&data)?;
```

CLI equivalent:
```bash
vectrust graph export --path ./data --output graph.json
vectrust graph import --path ./data --file graph.json
```

## Concurrent Access

`GraphIndex` is `Send + Sync` — safe to share across MCP tool handlers:

```rust
let db = Arc::new(vectrust::GraphIndex::open("./data")?);

// Pass to each MCP tool handler
let db_clone = Arc::clone(&db);
tool_handler.register("find_callers", move |params| {
    db_clone.cypher_with_params(
        "MATCH (caller)-[:CALLS]->(target:Function) WHERE target.name = $name RETURN caller.name",
        params,
    )
});
```

## Performance Expectations

| Operation | Target | Notes |
|-----------|--------|-------|
| Batch node creation | 84K nodes/sec | Via `create_nodes_batch` |
| Batch edge creation | 113K edges/sec | Via `create_edges_batch` |
| Property lookup (indexed) | <1ms | Via `create_property_index` |
| Property lookup (scan) | ~20-40ms at 1000 nodes | Without index |
| kNN search (cached HNSW) | <1ms for k=10 | After first query builds index |
| kNN search (first call) | 1-30s | HNSW index construction (one-time) |
| 1-hop traversal | <1ms | Via adjacency prefix scan |
| Variable-length path (1..3) | <50ms | BFS with cycle detection |
| MERGE (with index) | <1ms | Property index required for speed |
