// Copyright 2024-2026 Andrey Vasilevsky <anvanster@gmail.com>
// SPDX-License-Identifier: Apache-2.0

use tempfile::TempDir;
use vectrust::{GraphIndex, GraphValue, Result};

fn setup() -> (GraphIndex, TempDir) {
    let dir = TempDir::new().unwrap();
    let db = GraphIndex::open(dir.path()).unwrap();
    (db, dir)
}

// ─── Programmatic API ────────────────────────────────────────────

#[test]
fn test_graph_open_and_create_node() -> Result<()> {
    let (db, _dir) = setup();

    let node = db.create_node(&["Person"], serde_json::json!({"name": "Alice", "age": 30}))?;
    assert_eq!(node.labels, vec!["Person"]);
    assert_eq!(
        node.properties.get("name"),
        Some(&GraphValue::String("Alice".into()))
    );

    let retrieved = db.get_node(node.id)?.unwrap();
    assert_eq!(retrieved.id, node.id);
    assert_eq!(
        retrieved.properties.get("age"),
        Some(&GraphValue::Integer(30))
    );

    Ok(())
}

#[test]
fn test_graph_create_edge_and_traverse() -> Result<()> {
    let (db, _dir) = setup();

    let alice = db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))?;
    let bob = db.create_node(&["Person"], serde_json::json!({"name": "Bob"}))?;
    let edge = db.create_edge(
        alice.id,
        bob.id,
        "KNOWS",
        serde_json::json!({"since": 2020}),
    )?;

    assert_eq!(edge.source, alice.id);
    assert_eq!(edge.target, bob.id);
    assert_eq!(edge.rel_type, "KNOWS");

    let neighbors = db.neighbors(alice.id, Some("KNOWS"))?;
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].1.id, bob.id);

    Ok(())
}

#[test]
fn test_graph_nodes_by_label() -> Result<()> {
    let (db, _dir) = setup();

    db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))?;
    db.create_node(&["Person"], serde_json::json!({"name": "Bob"}))?;
    db.create_node(&["Document"], serde_json::json!({"title": "Paper"}))?;

    assert_eq!(db.nodes_by_label("Person")?.len(), 2);
    assert_eq!(db.nodes_by_label("Document")?.len(), 1);
    assert_eq!(db.nodes_by_label("Unknown")?.len(), 0);

    Ok(())
}

#[test]
fn test_graph_delete_with_detach() -> Result<()> {
    let (db, _dir) = setup();

    let alice = db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))?;
    let bob = db.create_node(&["Person"], serde_json::json!({"name": "Bob"}))?;
    db.create_edge(alice.id, bob.id, "KNOWS", serde_json::json!({}))?;

    // Can't delete without detach when edges exist
    assert!(db.delete_node(alice.id, false).is_err());

    // Can delete with detach
    db.delete_node(alice.id, true)?;
    assert!(db.get_node(alice.id)?.is_none());

    // Bob should still exist, but with no incoming edges
    assert!(db.get_node(bob.id)?.is_some());
    assert_eq!(db.neighbors(bob.id, None)?.len(), 0);

    Ok(())
}

// ─── Cypher Queries ──────────────────────────────────────────────

#[test]
fn test_cypher_create_and_match() -> Result<()> {
    let (db, _dir) = setup();

    db.cypher("CREATE (n:Person {name: 'Alice', age: 30})")?;
    db.cypher("CREATE (n:Person {name: 'Bob', age: 25})")?;

    let result = db.cypher("MATCH (n:Person) RETURN n.name AS name ORDER BY name")?;
    assert_eq!(result.columns, vec!["name"]);
    assert_eq!(result.rows.len(), 2);
    assert_eq!(
        result.rows[0].get("name"),
        Some(&GraphValue::String("Alice".into()))
    );
    assert_eq!(
        result.rows[1].get("name"),
        Some(&GraphValue::String("Bob".into()))
    );

    Ok(())
}

#[test]
fn test_cypher_where_filter() -> Result<()> {
    let (db, _dir) = setup();

    db.cypher("CREATE (n:Person {name: 'Alice', age: 30})")?;
    db.cypher("CREATE (n:Person {name: 'Bob', age: 25})")?;
    db.cypher("CREATE (n:Person {name: 'Carol', age: 35})")?;

    let result =
        db.cypher("MATCH (n:Person) WHERE n.age >= 30 RETURN n.name AS name ORDER BY name")?;
    assert_eq!(result.rows.len(), 2);
    assert_eq!(
        result.rows[0].get("name"),
        Some(&GraphValue::String("Alice".into()))
    );
    assert_eq!(
        result.rows[1].get("name"),
        Some(&GraphValue::String("Carol".into()))
    );

    Ok(())
}

#[test]
fn test_cypher_edge_traversal() -> Result<()> {
    let (db, _dir) = setup();

    let alice = db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))?;
    let bob = db.create_node(&["Person"], serde_json::json!({"name": "Bob"}))?;
    let carol = db.create_node(&["Person"], serde_json::json!({"name": "Carol"}))?;
    db.create_edge(alice.id, bob.id, "KNOWS", serde_json::json!({}))?;
    db.create_edge(alice.id, carol.id, "KNOWS", serde_json::json!({}))?;

    let result = db.cypher(
        "MATCH (a:Person)-[:KNOWS]->(b:Person) WHERE a.name = 'Alice' RETURN b.name AS friend ORDER BY friend"
    )?;
    assert_eq!(result.rows.len(), 2);
    assert_eq!(
        result.rows[0].get("friend"),
        Some(&GraphValue::String("Bob".into()))
    );
    assert_eq!(
        result.rows[1].get("friend"),
        Some(&GraphValue::String("Carol".into()))
    );

    Ok(())
}

#[test]
fn test_cypher_with_parameters() -> Result<()> {
    let (db, _dir) = setup();

    db.cypher("CREATE (n:Person {name: 'Alice', age: 30})")?;
    db.cypher("CREATE (n:Person {name: 'Bob', age: 25})")?;

    let result = db.cypher_with_params(
        "MATCH (n:Person) WHERE n.name = $name RETURN n.age AS age",
        serde_json::json!({"name": "Alice"}),
    )?;
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].get("age"), Some(&GraphValue::Integer(30)));

    Ok(())
}

#[test]
fn test_cypher_set_and_delete() -> Result<()> {
    let (db, _dir) = setup();

    db.cypher("CREATE (n:Person {name: 'Alice', age: 30})")?;

    // SET
    db.cypher("MATCH (n:Person) WHERE n.name = 'Alice' SET n.age = 31")?;
    let result = db.cypher("MATCH (n:Person) WHERE n.name = 'Alice' RETURN n.age AS age")?;
    assert_eq!(result.rows[0].get("age"), Some(&GraphValue::Integer(31)));

    // DELETE
    db.cypher("MATCH (n:Person) WHERE n.name = 'Alice' DETACH DELETE n")?;
    let result = db.cypher("MATCH (n:Person) RETURN n.name")?;
    assert!(result.rows.is_empty());

    Ok(())
}

#[test]
fn test_cypher_order_limit_skip() -> Result<()> {
    let (db, _dir) = setup();

    for i in 0..5 {
        db.cypher(&format!("CREATE (n:Item {{val: {}}})", i))?;
    }

    let result = db.cypher("MATCH (n:Item) RETURN n.val AS v ORDER BY v DESC LIMIT 3")?;
    assert_eq!(result.rows.len(), 3);
    assert_eq!(result.rows[0].get("v"), Some(&GraphValue::Integer(4)));
    assert_eq!(result.rows[2].get("v"), Some(&GraphValue::Integer(2)));

    let result = db.cypher("MATCH (n:Item) RETURN n.val AS v ORDER BY v SKIP 2 LIMIT 2")?;
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.rows[0].get("v"), Some(&GraphValue::Integer(2)));
    assert_eq!(result.rows[1].get("v"), Some(&GraphValue::Integer(3)));

    Ok(())
}

#[test]
fn test_cypher_aggregation() -> Result<()> {
    let (db, _dir) = setup();

    db.cypher("CREATE (n:Person {name: 'Alice', dept: 'Eng'})")?;
    db.cypher("CREATE (n:Person {name: 'Bob', dept: 'Eng'})")?;
    db.cypher("CREATE (n:Person {name: 'Carol', dept: 'Sales'})")?;

    // count(*)
    let result = db.cypher("MATCH (n:Person) RETURN count(*) AS total")?;
    assert_eq!(result.rows[0].get("total"), Some(&GraphValue::Integer(3)));

    // count with grouping
    let result =
        db.cypher("MATCH (n:Person) RETURN n.dept AS dept, count(*) AS c ORDER BY dept")?;
    assert_eq!(result.rows.len(), 2);
    assert_eq!(result.rows[0].get("c"), Some(&GraphValue::Integer(2)));
    assert_eq!(result.rows[1].get("c"), Some(&GraphValue::Integer(1)));

    // collect
    let result =
        db.cypher("MATCH (n:Person) WHERE n.dept = 'Eng' RETURN collect(n.name) AS names")?;
    if let Some(GraphValue::List(names)) = result.rows[0].get("names") {
        assert_eq!(names.len(), 2);
    } else {
        panic!("Expected list");
    }

    Ok(())
}

#[test]
fn test_cypher_variable_length_path() -> Result<()> {
    let (db, _dir) = setup();

    // Chain: A -> B -> C -> D
    let a = db.create_node(&["N"], serde_json::json!({"name": "A"}))?;
    let b = db.create_node(&["N"], serde_json::json!({"name": "B"}))?;
    let c = db.create_node(&["N"], serde_json::json!({"name": "C"}))?;
    let d = db.create_node(&["N"], serde_json::json!({"name": "D"}))?;
    db.create_edge(a.id, b.id, "NEXT", serde_json::json!({}))?;
    db.create_edge(b.id, c.id, "NEXT", serde_json::json!({}))?;
    db.create_edge(c.id, d.id, "NEXT", serde_json::json!({}))?;

    // 1..3 hops from A
    let result = db.cypher(
        "MATCH (a:N)-[:NEXT*1..3]->(b:N) WHERE a.name = 'A' RETURN b.name AS name ORDER BY name",
    )?;
    assert_eq!(result.rows.len(), 3); // B, C, D

    // Exactly 2 hops
    let result =
        db.cypher("MATCH (a:N)-[:NEXT*2..2]->(b:N) WHERE a.name = 'A' RETURN b.name AS name")?;
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get("name"),
        Some(&GraphValue::String("C".into()))
    );

    Ok(())
}

// ─── Vector Functions ────────────────────────────────────────────

#[test]
fn test_vector_similarity_in_cypher() -> Result<()> {
    let (db, _dir) = setup();

    db.create_node_with_vector(
        &["Doc"],
        serde_json::json!({"title": "AI"}),
        vec![1.0, 0.0, 0.0],
    )?;
    db.create_node_with_vector(
        &["Doc"],
        serde_json::json!({"title": "ML"}),
        vec![0.9, 0.1, 0.0],
    )?;
    db.create_node_with_vector(
        &["Doc"],
        serde_json::json!({"title": "Cooking"}),
        vec![0.0, 0.0, 1.0],
    )?;

    let result = db.cypher_with_params(
        "MATCH (n:Doc) WHERE vector_similarity(n.embedding, $q) > 0.5 RETURN n.title AS title ORDER BY title",
        serde_json::json!({"q": [1.0, 0.0, 0.0]}),
    )?;

    assert_eq!(result.rows.len(), 2); // AI and ML, not Cooking
    Ok(())
}

#[test]
fn test_call_nearest_knn() -> Result<()> {
    let (db, _dir) = setup();

    db.create_node_with_vector(
        &["Doc"],
        serde_json::json!({"title": "AI"}),
        vec![1.0, 0.0, 0.0],
    )?;
    db.create_node_with_vector(
        &["Doc"],
        serde_json::json!({"title": "ML"}),
        vec![0.9, 0.1, 0.0],
    )?;
    db.create_node_with_vector(
        &["Doc"],
        serde_json::json!({"title": "Cooking"}),
        vec![0.0, 0.0, 1.0],
    )?;

    let result = db.cypher_with_params(
        "CALL vectrust.nearest('embedding', $q, 2) YIELD node, score RETURN node.title AS title, score ORDER BY score DESC",
        serde_json::json!({"q": [1.0, 0.0, 0.0]}),
    )?;

    assert_eq!(result.rows.len(), 2);
    assert_eq!(
        result.rows[0].get("title"),
        Some(&GraphValue::String("AI".into()))
    );

    Ok(())
}

#[test]
fn test_knn_plus_graph_traversal() -> Result<()> {
    let (db, _dir) = setup();

    // Build: Person -AUTHORED-> Document (with vector)
    let alice = db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))?;
    let paper = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "AI Paper"}),
        vec![1.0, 0.0, 0.0],
    )?;
    db.create_edge(alice.id, paper.id, "AUTHORED", serde_json::json!({}))?;

    db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "Unrelated"}),
        vec![0.0, 0.0, 1.0],
    )?;

    // kNN → graph traversal
    let result = db.cypher_with_params(
        "CALL vectrust.nearest('embedding', $q, 1) YIELD node, score \
         MATCH (author:Person)-[:AUTHORED]->(node) \
         RETURN author.name AS name, node.title AS doc",
        serde_json::json!({"q": [1.0, 0.0, 0.0]}),
    )?;

    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get("name"),
        Some(&GraphValue::String("Alice".into()))
    );
    assert_eq!(
        result.rows[0].get("doc"),
        Some(&GraphValue::String("AI Paper".into()))
    );

    Ok(())
}

// ─── Mixed API ───────────────────────────────────────────────────

#[test]
fn test_mixed_programmatic_and_cypher() -> Result<()> {
    let (db, _dir) = setup();

    // Create programmatically
    let alice = db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))?;
    let bob = db.create_node(&["Person"], serde_json::json!({"name": "Bob"}))?;
    db.create_edge(alice.id, bob.id, "KNOWS", serde_json::json!({}))?;

    // Query with Cypher
    let result =
        db.cypher("MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name AS from, b.name AS to")?;
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get("from"),
        Some(&GraphValue::String("Alice".into()))
    );
    assert_eq!(
        result.rows[0].get("to"),
        Some(&GraphValue::String("Bob".into()))
    );

    // Modify with Cypher
    db.cypher("MATCH (n:Person) WHERE n.name = 'Alice' SET n.title = 'Engineer'")?;

    // Verify programmatically
    let alice_updated = db.get_node(alice.id)?.unwrap();
    assert_eq!(
        alice_updated.properties.get("title"),
        Some(&GraphValue::String("Engineer".into()))
    );

    Ok(())
}

// ─── PRD Example Query ──────────────────────────────────────────

#[test]
fn test_prd_example_document_graph() -> Result<()> {
    let (db, _dir) = setup();

    let overview = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "AI Overview", "topic": "AI"}),
        vec![1.0, 0.0, 0.0],
    )?;
    let deep_learning = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "Deep Learning", "topic": "AI"}),
        vec![0.9, 0.1, 0.0],
    )?;
    let _cooking = db.create_node_with_vector(
        &["Document"],
        serde_json::json!({"title": "Cooking Guide", "topic": "Food"}),
        vec![0.0, 0.0, 1.0],
    )?;
    db.create_edge(
        overview.id,
        deep_learning.id,
        "REFERENCES",
        serde_json::json!({}),
    )?;

    // PRD query: graph traversal + filter
    let result = db.cypher(
        "MATCH (doc:Document)-[:REFERENCES]->(ref:Document) \
         WHERE doc.topic = 'AI' \
         RETURN ref.title AS title \
         ORDER BY title LIMIT 5",
    )?;
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].get("title"),
        Some(&GraphValue::String("Deep Learning".into()))
    );

    // PRD query: vector similarity
    let result = db.cypher_with_params(
        "MATCH (n:Document) \
         RETURN n.title AS title, vector_similarity(n.embedding, $query) AS score \
         ORDER BY score DESC LIMIT 2",
        serde_json::json!({"query": [1.0, 0.0, 0.0]}),
    )?;
    assert_eq!(result.rows.len(), 2);
    assert_eq!(
        result.rows[0].get("title"),
        Some(&GraphValue::String("AI Overview".into()))
    );

    Ok(())
}
