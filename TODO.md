# Vectrust TODO

Post-v0.2.0 work driven by CodeGraph migration requirements.

## Performance (blocking migration)

- [x] **HNSW-accelerated kNN**: Replaced brute-force with cached HnswIndex. Lazily built on first query, invalidated on vector insert.
- [ ] **Benchmark with realistic CodeGraph data**: 5000-6000 nodes, 768d vectors (Jina Code V2), measure create/query/traverse latency against targets: <100ms navigation, <500ms analysis.
- [x] **Batch node/edge creation**: `create_nodes_batch()` / `create_edges_batch()` via RocksDB WriteBatch.
- [ ] **768d cosine similarity optimization**: Profile `vector_similarity()` at 768 dimensions. Consider SIMD if needed — this is the hot path for CodeGraph's 35 MCP tools.

## API Completeness (needed for migration)

- [ ] **Batch Cypher execution**: Execute multiple statements in one call. CodeGraph indexes entire codebases in one pass — needs atomic multi-CREATE.
- [x] **MERGE (upsert)**: `MERGE (n:Function {name: $name}) ON CREATE SET ... ON MATCH SET ...` with idempotent semantics.
- [ ] **OPTIONAL MATCH**: `OPTIONAL MATCH (n)-[:CALLS]->(m) RETURN n, m`. Needed when traversal targets may not exist (e.g., external function calls).
- [ ] **Multiple labels per node**: Verify nodes work with `[:Function:Public]` style multi-label patterns. CodeGraph uses visibility as a secondary label.

## Migration Support

- [ ] **Migration guide**: Document concrete CodeGraph API → Vectrust mapping with examples for each of the 35 MCP tools' query patterns.
- [ ] **Import from JSON**: `vectrust graph import --path ./data --file graph.json` for bootstrapping from CodeGraph's existing graph dump.
- [ ] **Embedding integration**: Add optional `fastembed` or generic embedding callback so nodes can be auto-embedded on creation. CodeGraph uses Jina Code V2 (768d).

## Reliability

- [ ] **Write-ahead log for graph mutations**: Currently graph writes go directly to RocksDB CFs. Add WAL for crash recovery during bulk imports.
- [ ] **Index rebuilding**: `vectrust graph reindex --path ./data` to rebuild label/reltype/adjacency indexes from node/edge records if they get corrupted.
- [ ] **Concurrent read safety**: Verify `GraphIndex` is safe for concurrent reads from multiple MCP tool handlers (35 tools, potentially parallel).

## Phase 2 Cypher (nice-to-have)

- [ ] UNWIND, FOREACH
- [ ] CASE WHEN expressions
- [ ] UNION
- [ ] Subqueries, EXISTS
- [ ] SHORTEST PATH
- [ ] Full-text search integration
