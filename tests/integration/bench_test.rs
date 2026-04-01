// Copyright 2024-2026 Andrey Vasilevsky <anvanster@gmail.com>
// SPDX-License-Identifier: Apache-2.0

use std::time::Instant;
use tempfile::TempDir;
use vectrust::{GraphIndex, GraphValue};

/// Benchmark at CodeGraph scale.
///
/// In debug mode uses 1000 nodes/128d for fast CI. For realistic numbers:
/// ```sh
/// cargo test --release -p integration-tests -- bench_codegraph_scale --nocapture
/// ```
#[test]
fn bench_codegraph_scale() {
    let dir = TempDir::new().unwrap();
    let db = GraphIndex::open(dir.path()).unwrap();

    // Use smaller sizes in debug mode to keep test <30s
    let (node_count, vector_dim, edge_count) = if cfg!(debug_assertions) {
        (1000, 128, 2000)
    } else {
        (6000, 768, 12000)
    };

    println!("\n=== CodeGraph-Scale Benchmark ===");
    println!(
        "Nodes: {}, Edges: {}, Vector dims: {}\n",
        node_count, edge_count, vector_dim
    );

    // ── 1. Batch node creation ───────────────────────────────────
    let start = Instant::now();
    let mut node_ids = Vec::with_capacity(node_count);

    // Create in batches of 500
    for batch_start in (0..node_count).step_by(500) {
        let batch_end = (batch_start + 500).min(node_count);
        let batch: Vec<(&[&str], serde_json::Value)> = (batch_start..batch_end)
            .map(|i| {
                let labels: &[&str] = &["Function"];
                let props = serde_json::json!({
                    "name": format!("fn_{}", i),
                    "file": format!("src/file_{}.rs", i / 10),
                    "complexity": (i % 20) + 1,
                });
                (labels, props)
            })
            .collect();
        let ids = db.create_nodes_batch(&batch).unwrap();
        node_ids.extend(ids);
    }
    let node_time = start.elapsed();
    println!(
        "1. Create {} nodes: {:.1}ms ({:.0} nodes/sec)",
        node_count,
        node_time.as_secs_f64() * 1000.0,
        node_count as f64 / node_time.as_secs_f64()
    );

    // ── 2. Add vectors to nodes ──────────────────────────────────
    let start = Instant::now();
    for (i, id) in node_ids.iter().enumerate() {
        // Generate a pseudo-random but deterministic 768d vector
        let vector: Vec<f32> = (0..vector_dim)
            .map(|d| ((i * 7 + d * 13) % 1000) as f32 / 1000.0)
            .collect();
        db.create_node_with_vector(
            &["Function"],
            serde_json::json!({"name": format!("fn_{}", i)}),
            vector,
        )
        .unwrap();
    }
    let vector_time = start.elapsed();
    println!(
        "2. Add {} vectors ({}d): {:.1}ms ({:.0} vectors/sec)",
        node_count,
        vector_dim,
        vector_time.as_secs_f64() * 1000.0,
        node_count as f64 / vector_time.as_secs_f64()
    );

    // ── 3. Batch edge creation ───────────────────────────────────
    let start = Instant::now();
    for batch_start in (0..edge_count).step_by(500) {
        let batch_end = (batch_start + 500).min(edge_count);
        let batch: Vec<(uuid::Uuid, uuid::Uuid, &str, serde_json::Value)> = (batch_start
            ..batch_end)
            .map(|i| {
                let src = node_ids[i % node_count];
                let tgt = node_ids[(i * 7 + 3) % node_count];
                let rel = if i % 3 == 0 {
                    "CALLS"
                } else if i % 3 == 1 {
                    "IMPORTS"
                } else {
                    "REFERENCES"
                };
                (src, tgt, rel, serde_json::json!({}))
            })
            .collect();
        db.create_edges_batch(&batch).unwrap();
    }
    let edge_time = start.elapsed();
    println!(
        "3. Create {} edges: {:.1}ms ({:.0} edges/sec)",
        edge_count,
        edge_time.as_secs_f64() * 1000.0,
        edge_count as f64 / edge_time.as_secs_f64()
    );

    // ── 4. Simple MATCH query ────────────────────────────────────
    let start = Instant::now();
    let result = db
        .cypher("MATCH (n:Function) WHERE n.complexity > 15 RETURN n.name LIMIT 10")
        .unwrap();
    let match_time = start.elapsed();
    println!(
        "4. MATCH + WHERE + LIMIT: {:.2}ms ({} results)",
        match_time.as_secs_f64() * 1000.0,
        result.rows.len()
    );
    assert!(
        match_time.as_millis() < 1000,
        "MATCH too slow: {}ms",
        match_time.as_millis()
    );

    // ── 5. Edge traversal ────────────────────────────────────────
    let start = Instant::now();
    let result = db
        .cypher_with_params(
            "MATCH (a:Function)-[:CALLS]->(b:Function) WHERE a.name = $name RETURN b.name LIMIT 10",
            serde_json::json!({"name": "fn_0"}),
        )
        .unwrap();
    let traverse_time = start.elapsed();
    println!(
        "5. 1-hop traversal: {:.2}ms ({} results)",
        traverse_time.as_secs_f64() * 1000.0,
        result.rows.len()
    );
    // Note: without property indexes, WHERE a.name = $name requires full label scan.
    // With a property index this would be <1ms. For now accept <500ms.
    assert!(
        traverse_time.as_millis() < 500,
        "Traversal too slow: {}ms",
        traverse_time.as_millis()
    );

    // ── 6. kNN search (HNSW) ─────────────────────────────────────
    let query_vec: Vec<f32> = (0..vector_dim)
        .map(|d| (d % 1000) as f32 / 1000.0)
        .collect();
    // First call builds the HNSW index
    let start = Instant::now();
    let result = db
        .cypher_with_params(
            "CALL vectrust.nearest('embedding', $q, 10) YIELD node, score RETURN node.name, score",
            serde_json::json!({"q": query_vec.clone()}),
        )
        .unwrap();
    let knn_first_time = start.elapsed();
    println!(
        "6. kNN (k=10, first call, builds HNSW): {:.1}ms ({} results)",
        knn_first_time.as_secs_f64() * 1000.0,
        result.rows.len()
    );

    // Second call uses cached index
    let start = Instant::now();
    let result = db
        .cypher_with_params(
            "CALL vectrust.nearest('embedding', $q, 10) YIELD node, score RETURN node.name, score",
            serde_json::json!({"q": query_vec}),
        )
        .unwrap();
    let knn_cached_time = start.elapsed();
    println!(
        "7. kNN (k=10, cached HNSW): {:.2}ms ({} results)",
        knn_cached_time.as_secs_f64() * 1000.0,
        result.rows.len()
    );
    assert!(
        knn_cached_time.as_millis() < 100,
        "Cached kNN too slow: {}ms",
        knn_cached_time.as_millis()
    );

    // ── 7. Aggregation ───────────────────────────────────────────
    let start = Instant::now();
    let result = db
        .cypher(
            "MATCH (n:Function) RETURN n.file AS file, count(*) AS cnt ORDER BY cnt DESC LIMIT 5",
        )
        .unwrap();
    let agg_time = start.elapsed();
    println!(
        "8. Aggregation (GROUP BY file): {:.1}ms ({} groups)",
        agg_time.as_secs_f64() * 1000.0,
        result.rows.len()
    );

    // ── 8. MERGE ─────────────────────────────────────────────────
    let start = Instant::now();
    for i in 0..100 {
        db.cypher(&format!("MERGE (n:Function {{name: 'fn_{}'}})", i))
            .unwrap();
    }
    let merge_time = start.elapsed();
    println!(
        "9. 100x MERGE (upsert): {:.1}ms ({:.2}ms/op)",
        merge_time.as_secs_f64() * 1000.0,
        merge_time.as_secs_f64() * 1000.0 / 100.0
    );

    // ── 9. Stats ─────────────────────────────────────────────────
    let stats = db.graph_stats().unwrap();
    println!(
        "\nFinal stats: {} nodes, {} edges, vectors: {}",
        stats.node_count, stats.edge_count, stats.has_vectors
    );

    println!("\n=== Benchmark Complete ===\n");
}
