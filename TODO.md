# Vectrust TODO

Post-v0.2.0 work driven by CodeGraph migration requirements.

## Performance (blocking migration)

- [x] **HNSW-accelerated kNN**: Replaced brute-force with cached HnswIndex. Lazily built on first query, invalidated on vector insert.
- [x] **Batch node/edge creation**: `create_nodes_batch()` / `create_edges_batch()` via RocksDB WriteBatch.
- [x] **Property indexes**: `create_property_index(label, property)` for O(1) WHERE/MERGE lookups.
- [x] **Benchmark (debug mode)**: 1000 nodes/128d — 84K nodes/sec, 0.83ms cached kNN, 113K edges/sec.
- [ ] **Release-mode benchmark**: Run at full 6000 nodes/768d scale with `cargo test --release`. Debug benchmark exists.
- [ ] **768d cosine similarity optimization**: Profile `vector_similarity()` at 768 dimensions. Consider SIMD if needed — hot path for 35 MCP tools.

## API Completeness (needed for migration)

- [x] **MERGE (upsert)**: `MERGE (n:Function {name: $name}) ON CREATE SET ... ON MATCH SET ...`
- [x] **OPTIONAL MATCH**: Preserves rows with null bindings when no match found.
- [x] **Batch Cypher execution**: `cypher_batch()` / `cypher_batch_with_params()` for multi-statement bulk indexing.
- [x] **Multiple labels per node**: `CREATE (n:Function:Public)`, `MATCH (n:Function)`, `MATCH (n:Function:Public)` all verified working.
- [x] **Concurrent read safety**: `GraphIndex` is `Send + Sync`. Tested with 10 concurrent reader threads and mixed reader/writer threads.

## Migration Support

- [ ] **Migration guide**: Document concrete CodeGraph API → Vectrust mapping.
- [ ] **Import from JSON**: `vectrust graph import --path ./data --file graph.json`.
- [ ] **Embedding integration**: Optional callback for auto-embedding nodes on creation.

## Reliability

- [ ] **Write-ahead log**: WAL for crash recovery during bulk imports.
- [ ] **Index rebuilding**: `vectrust graph reindex --path ./data` CLI command.

## Phase 2 Cypher (nice-to-have)

- [ ] UNWIND, FOREACH
- [ ] CASE WHEN expressions
- [ ] UNION
- [ ] Subqueries, EXISTS
- [ ] SHORTEST PATH
- [ ] Full-text search integration
