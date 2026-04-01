// Copyright 2024-2026 Andrey Vasilevsky <anvanster@gmail.com>
// SPDX-License-Identifier: Apache-2.0

/// Graph + Vector database example using Vectrust.
///
/// Demonstrates:
/// - Creating nodes with labels and properties
/// - Creating edges between nodes
/// - Cypher queries for graph traversal
/// - Vector similarity search within graph queries
/// - CALL vectrust.nearest for kNN search
///
/// Run with: cargo run --bin graph
use vectrust::GraphIndex;

fn main() -> vectrust::Result<()> {
    let dir = tempfile::TempDir::new().unwrap();
    let db = GraphIndex::open(dir.path())?;

    println!("=== Vectrust Graph + Vector Database ===\n");

    // ── 1. Create a knowledge graph ──────────────────────────────

    println!("1. Building a document knowledge graph...\n");

    let alice = db.create_node(
        &["Person"],
        serde_json::json!({
            "name": "Alice",
            "role": "Researcher"
        }),
    )?;

    let bob = db.create_node(
        &["Person"],
        serde_json::json!({
            "name": "Bob",
            "role": "Engineer"
        }),
    )?;

    // Documents with embedding vectors
    let ai_paper = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "Attention Is All You Need", "topic": "AI"}),
        vec![0.9, 0.8, 0.1, 0.0], // AI-related vector
    )?;

    let ml_paper = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "Deep Residual Learning", "topic": "AI"}),
        vec![0.85, 0.75, 0.15, 0.05], // Similar AI vector
    )?;

    let _cooking = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "The Art of French Cooking", "topic": "Food"}),
        vec![0.0, 0.1, 0.9, 0.8], // Very different vector
    )?;

    // Create relationships
    db.create_edge(alice.id, ai_paper.id, "AUTHORED", serde_json::json!({}))?;
    db.create_edge(bob.id, ml_paper.id, "AUTHORED", serde_json::json!({}))?;
    db.create_edge(
        ai_paper.id,
        ml_paper.id,
        "REFERENCES",
        serde_json::json!({}),
    )?;
    db.create_edge(
        alice.id,
        bob.id,
        "COLLABORATES_WITH",
        serde_json::json!({"since": 2020}),
    )?;

    println!("   Created {} people, {} documents, and edges\n", 2, 3);

    // ── 2. Graph queries with Cypher ─────────────────────────────

    println!("2. Cypher graph queries:\n");

    // Who authored what?
    let result = db.cypher(
        "MATCH (p:Person)-[:AUTHORED]->(d:Document) \
         RETURN p.name AS author, d.title AS paper",
    )?;
    println!("   Authors and papers:");
    for row in &result.rows {
        println!(
            "     {} wrote \"{}\"",
            row.get("author").and_then(|v| v.as_str()).unwrap_or("?"),
            row.get("paper").and_then(|v| v.as_str()).unwrap_or("?"),
        );
    }

    // What does Alice's paper reference?
    let result = db.cypher(
        "MATCH (p:Person)-[:AUTHORED]->(d:Document)-[:REFERENCES]->(ref:Document) \
         WHERE p.name = 'Alice' \
         RETURN ref.title AS referenced",
    )?;
    println!("\n   Alice's paper references:");
    for row in &result.rows {
        println!(
            "     \"{}\"",
            row.get("referenced")
                .and_then(|v| v.as_str())
                .unwrap_or("?"),
        );
    }

    // ── 3. Aggregation ───────────────────────────────────────────

    println!("\n3. Aggregation:\n");

    let result = db.cypher(
        "MATCH (d:Document) RETURN d.topic AS topic, count(*) AS total ORDER BY total DESC",
    )?;
    println!("   Documents by topic:");
    for row in &result.rows {
        println!(
            "     {}: {}",
            row.get("topic").and_then(|v| v.as_str()).unwrap_or("?"),
            row.get("total").and_then(|v| v.as_i64()).unwrap_or(0),
        );
    }

    // ── 4. Vector similarity in Cypher ───────────────────────────

    println!("\n4. Vector similarity search:\n");

    let result = db.cypher_with_params(
        "MATCH (n:Document) \
         RETURN n.title AS title, vector_similarity(n.embedding, $query) AS similarity \
         ORDER BY similarity DESC",
        serde_json::json!({"query": [0.9, 0.8, 0.1, 0.0]}),
    )?;
    println!("   Documents ranked by similarity to AI query:");
    for row in &result.rows {
        let title = row.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let sim = row
            .get("similarity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        println!("     {:.4}  \"{}\"", sim, title);
    }

    // ── 5. CALL vectrust.nearest (kNN) ───────────────────────────

    println!("\n5. kNN search with CALL vectrust.nearest:\n");

    let result = db.cypher_with_params(
        "CALL vectrust.nearest('embedding', $q, 2) YIELD node, score \
         RETURN node.title AS title, score \
         ORDER BY score DESC",
        serde_json::json!({"q": [0.9, 0.8, 0.1, 0.0]}),
    )?;
    println!("   Top 2 nearest documents:");
    for row in &result.rows {
        let title = row.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let score = row.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        println!("     {:.4}  \"{}\"", score, title);
    }

    // ── 6. Combined: kNN + graph traversal ───────────────────────

    println!("\n6. Combined kNN + graph traversal:\n");

    let result = db.cypher_with_params(
        "CALL vectrust.nearest('embedding', $q, 1) YIELD node, score \
         MATCH (author:Person)-[:AUTHORED]->(node) \
         RETURN author.name AS author, node.title AS paper, score",
        serde_json::json!({"q": [0.9, 0.8, 0.1, 0.0]}),
    )?;
    println!("   Nearest paper and its author:");
    for row in &result.rows {
        println!(
            "     \"{}\" by {} (score: {:.4})",
            row.get("paper").and_then(|v| v.as_str()).unwrap_or("?"),
            row.get("author").and_then(|v| v.as_str()).unwrap_or("?"),
            row.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0),
        );
    }

    // ── 7. Filter: graph + vector ────────────────────────────────

    println!("\n7. Graph filter + vector ranking:\n");

    let result = db.cypher_with_params(
        "MATCH (d:Document) \
         WHERE d.topic = 'AI' AND vector_similarity(d.embedding, $q) > 0.8 \
         RETURN d.title AS title, vector_similarity(d.embedding, $q) AS sim \
         ORDER BY sim DESC",
        serde_json::json!({"q": [0.9, 0.8, 0.1, 0.0]}),
    )?;
    println!("   AI documents with similarity > 0.8:");
    for row in &result.rows {
        let title = row.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let sim = row.get("sim").and_then(|v| v.as_f64()).unwrap_or(0.0);
        println!("     {:.4}  \"{}\"", sim, title);
    }

    println!("\nDone!");
    Ok(())
}
