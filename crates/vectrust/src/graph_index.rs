use rocksdb::DB;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use vectrust_core::{
    GraphEdge, GraphNode, GraphQueryResult, GraphStats, GraphValue, Result, VectorOps, VectraError,
};
use vectrust_cypher::CypherError;
use vectrust_graph::{GraphExecutor, GraphStorage};

const METADATA_CF: &str = "metadata";
const VECTOR_INDEX_CF: &str = "vector_index";

/// High-level graph + vector database with Cypher query support.
///
/// Uses a single shared RocksDB instance for both graph and vector data.
///
/// ```ignore
/// let db = GraphIndex::open("./data")?;
///
/// // Graph: Cypher queries
/// db.cypher("CREATE (n:Person {name: 'Alice'})")?;
/// let results = db.cypher("MATCH (p:Person) RETURN p.name")?;
///
/// // Vector: store and search embeddings
/// let doc = db.create_node_with_vector(&["Doc"], json!({"title": "AI"}), vec![0.1, 0.2])?;
///
/// // Combined: graph traversal + vector similarity
/// db.cypher_with_params(
///     "MATCH (n:Doc) WHERE vector_similarity(n.embedding, $q) > 0.8 RETURN n",
///     json!({"q": [0.1, 0.2]}),
/// )?;
/// ```
pub struct GraphIndex {
    storage: GraphStorage,
    /// Shared DB handle for vector operations (None when using graph-only storage)
    shared_db: Option<Arc<DB>>,
    #[allow(dead_code)]
    path: PathBuf,
}

impl GraphIndex {
    /// Open or create a graph + vector database at the given path.
    ///
    /// Uses a single shared RocksDB instance with all column families
    /// for both graph operations (nodes, edges, Cypher) and vector
    /// operations (insert, search, similarity).
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let (storage, db) = GraphStorage::open_shared_db(&path)?;
        Ok(Self {
            storage,
            shared_db: Some(db),
            path,
        })
    }

    // ─── Cypher API ──────────────────────────────────────────────

    /// Execute a Cypher query and return results.
    pub fn cypher(&self, query: &str) -> Result<GraphQueryResult> {
        self.cypher_with_params(query, serde_json::Value::Null)
    }

    /// Execute a Cypher query with parameter bindings.
    ///
    /// Parameters are passed as a JSON object and referenced in the query as `$name`.
    pub fn cypher_with_params(
        &self,
        query: &str,
        params: serde_json::Value,
    ) -> Result<GraphQueryResult> {
        let stmt = vectrust_cypher::parse(query).map_err(cypher_err)?;

        let params = match params {
            serde_json::Value::Object(obj) => obj
                .into_iter()
                .map(|(k, v)| (k, GraphValue::from(v)))
                .collect(),
            serde_json::Value::Null => HashMap::new(),
            _ => {
                return Err(VectraError::Cypher {
                    message: "Parameters must be a JSON object or null".to_string(),
                });
            }
        };

        let executor = GraphExecutor::new(&self.storage, params);
        executor.execute(&stmt)
    }

    // ─── Programmatic Node API ───────────────────────────────────

    /// Create a node with labels and properties. Returns the node.
    pub fn create_node(&self, labels: &[&str], properties: serde_json::Value) -> Result<GraphNode> {
        let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        let props = json_to_props(properties)?;
        let id = self.storage.create_node(&labels, props)?;
        self.storage
            .get_node(id)?
            .ok_or(VectraError::NodeNotFound { id: id.to_string() })
    }

    /// Create a node with labels, properties, and an embedding vector.
    pub fn create_node_with_vector(
        &self,
        labels: &[&str],
        properties: serde_json::Value,
        vector: Vec<f32>,
    ) -> Result<GraphNode> {
        let labels: Vec<String> = labels.iter().map(|s| s.to_string()).collect();
        let props = json_to_props(properties)?;
        let id = self
            .storage
            .create_node_with_vector(&labels, props, vector)?;
        self.storage
            .get_node(id)?
            .ok_or(VectraError::NodeNotFound { id: id.to_string() })
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: Uuid) -> Result<Option<GraphNode>> {
        self.storage.get_node(id)
    }

    /// Get the vector associated with a node.
    pub fn get_node_vector(&self, id: Uuid) -> Result<Option<Vec<f32>>> {
        self.storage.get_node_vector(id)
    }

    /// Delete a node. If `detach` is true, also deletes all connected edges.
    pub fn delete_node(&self, id: Uuid, detach: bool) -> Result<()> {
        self.storage.delete_node(id, detach)
    }

    /// Set a property on a node.
    pub fn set_node_property(&self, id: Uuid, key: &str, value: GraphValue) -> Result<()> {
        self.storage.set_node_property(id, key, value)
    }

    // ─── Programmatic Edge API ───────────────────────────────────

    /// Create an edge between two nodes. Returns the edge.
    pub fn create_edge(
        &self,
        source: Uuid,
        target: Uuid,
        rel_type: &str,
        properties: serde_json::Value,
    ) -> Result<GraphEdge> {
        let props = json_to_props(properties)?;
        let id = self.storage.create_edge(source, target, rel_type, props)?;
        self.storage
            .get_edge(id)?
            .ok_or(VectraError::EdgeNotFound { id: id.to_string() })
    }

    /// Get an edge by ID.
    pub fn get_edge(&self, id: Uuid) -> Result<Option<GraphEdge>> {
        self.storage.get_edge(id)
    }

    // ─── Query helpers ───────────────────────────────────────────

    /// Get all nodes with a given label.
    pub fn nodes_by_label(&self, label: &str) -> Result<Vec<GraphNode>> {
        let ids = self.storage.nodes_by_label(label)?;
        ids.into_iter()
            .filter_map(|id| self.storage.get_node(id).transpose())
            .collect()
    }

    /// Get outgoing neighbors of a node, optionally filtered by relationship type.
    pub fn neighbors(
        &self,
        node_id: Uuid,
        rel_type: Option<&str>,
    ) -> Result<Vec<(GraphEdge, GraphNode)>> {
        let types: Vec<String> = rel_type.map(|t| vec![t.to_string()]).unwrap_or_default();
        self.storage.expand_out(node_id, &types)
    }

    // ─── Batch operations ─────────────────────────────────────────

    /// Batch-create multiple nodes. Significantly faster than individual creates
    /// for bulk imports. Returns created node IDs.
    pub fn create_nodes_batch(&self, nodes: &[(&[&str], serde_json::Value)]) -> Result<Vec<Uuid>> {
        let batch: Vec<(Vec<String>, HashMap<String, GraphValue>)> = nodes
            .iter()
            .map(|(labels, props)| {
                let labels = labels.iter().map(|s| s.to_string()).collect();
                let props = json_to_props(props.clone()).unwrap_or_default();
                (labels, props)
            })
            .collect();
        self.storage.create_nodes_batch(&batch)
    }

    /// Batch-create multiple edges. Returns created edge IDs.
    pub fn create_edges_batch(
        &self,
        edges: &[(Uuid, Uuid, &str, serde_json::Value)],
    ) -> Result<Vec<Uuid>> {
        let batch: Vec<(Uuid, Uuid, String, HashMap<String, GraphValue>)> = edges
            .iter()
            .map(|(src, tgt, rel, props)| {
                let props = json_to_props(props.clone()).unwrap_or_default();
                (*src, *tgt, rel.to_string(), props)
            })
            .collect();
        self.storage.create_edges_batch(&batch)
    }

    // ─── Indexes ──────────────────────────────────────────────────

    /// Create a property index for fast lookups.
    /// After calling this, `MATCH (n:Label {property: value})` and
    /// `MERGE (n:Label {property: value})` use O(1) index lookup instead of full scan.
    pub fn create_property_index(&self, label: &str, property: &str) -> Result<()> {
        self.storage.create_property_index(label, property)
    }

    // ─── Stats ────────────────────────────────────────────────────

    /// Get graph database statistics.
    pub fn graph_stats(&self) -> Result<GraphStats> {
        let node_count = self.storage.all_nodes()?.len();
        let edge_count = self.storage.edge_count()?;
        let labels = self.storage.all_labels()?;
        let relationship_types = self.storage.all_relationship_types()?;
        let has_vectors = self.storage.has_vectors()?;

        Ok(GraphStats {
            node_count,
            edge_count,
            labels,
            relationship_types,
            has_vectors,
        })
    }

    // ─── Vector operations (shared DB) ───────────────────────────

    /// Insert a vector item into the shared database.
    ///
    /// Stores vector metadata in the `metadata` CF and raw vector data
    /// in the `vector_index` CF, compatible with the LocalIndex format.
    pub fn insert_vector(
        &self,
        id: Uuid,
        vector: &[f32],
        metadata: serde_json::Value,
    ) -> Result<()> {
        let db = self.shared_db().ok_or_else(|| VectraError::Storage {
            message: "Vector operations require shared storage".into(),
        })?;

        if !VectorOps::is_valid_vector(vector) {
            return Err(VectraError::VectorValidation {
                message: "Vector contains NaN or infinite values".into(),
            });
        }

        let cf_meta = db
            .cf_handle(METADATA_CF)
            .ok_or_else(|| VectraError::StorageError {
                message: "metadata CF not found".into(),
            })?;
        let cf_vec = db
            .cf_handle(VECTOR_INDEX_CF)
            .ok_or_else(|| VectraError::StorageError {
                message: "vector_index CF not found".into(),
            })?;

        // Store metadata
        let meta_bytes = serde_json::to_vec(&metadata)?;
        db.put_cf(&cf_meta, id.to_string().as_bytes(), &meta_bytes)?;

        // Store vector as raw f32 bytes
        let vec_bytes = vector_to_bytes(vector);
        db.put_cf(&cf_vec, id.to_string().as_bytes(), &vec_bytes)?;

        Ok(())
    }

    /// Get a vector by ID from the shared database.
    pub fn get_vector(&self, id: Uuid) -> Result<Option<Vec<f32>>> {
        let db = self.shared_db().ok_or_else(|| VectraError::Storage {
            message: "Vector operations require shared storage".into(),
        })?;

        let cf_vec = db
            .cf_handle(VECTOR_INDEX_CF)
            .ok_or_else(|| VectraError::StorageError {
                message: "vector_index CF not found".into(),
            })?;

        match db.get_cf(&cf_vec, id.to_string().as_bytes())? {
            Some(bytes) => Ok(Some(bytes_to_vector(&bytes))),
            None => Ok(None),
        }
    }

    /// Search for the k nearest vectors by cosine similarity.
    /// Returns (id, score) pairs sorted by descending similarity.
    pub fn query_vectors(&self, query: &[f32], k: usize) -> Result<Vec<(Uuid, f32)>> {
        let db = self.shared_db().ok_or_else(|| VectraError::Storage {
            message: "Vector operations require shared storage".into(),
        })?;

        let cf_vec = db
            .cf_handle(VECTOR_INDEX_CF)
            .ok_or_else(|| VectraError::StorageError {
                message: "vector_index CF not found".into(),
            })?;

        let iter = db.iterator_cf(&cf_vec, rocksdb::IteratorMode::Start);
        let mut scored: Vec<(Uuid, f32)> = Vec::new();

        for item in iter {
            let (key, value) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            if let Ok(id) = Uuid::parse_str(key_str) {
                let vec = bytes_to_vector(&value);
                if vec.len() == query.len() {
                    let score = VectorOps::cosine_similarity(&vec, query);
                    scored.push((id, score));
                }
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        Ok(scored)
    }

    fn shared_db(&self) -> Option<&Arc<DB>> {
        self.shared_db.as_ref()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────

fn json_to_props(value: serde_json::Value) -> Result<HashMap<String, GraphValue>> {
    match value {
        serde_json::Value::Object(obj) => Ok(obj
            .into_iter()
            .map(|(k, v)| (k, GraphValue::from(v)))
            .collect()),
        serde_json::Value::Null => Ok(HashMap::new()),
        _ => Err(VectraError::Graph {
            message: "Properties must be a JSON object or null".to_string(),
        }),
    }
}

fn cypher_err(e: CypherError) -> VectraError {
    VectraError::Cypher {
        message: e.to_string(),
    }
}

fn vector_to_bytes(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for &val in vector {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

fn bytes_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (GraphIndex, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = GraphIndex::open(dir.path()).unwrap();
        (db, dir)
    }

    #[test]
    fn test_programmatic_crud() {
        let (db, _dir) = setup();

        let alice = db
            .create_node(&["Person"], serde_json::json!({"name": "Alice", "age": 30}))
            .unwrap();
        assert_eq!(alice.labels, vec!["Person"]);
        assert_eq!(
            alice.properties.get("name"),
            Some(&GraphValue::String("Alice".into()))
        );

        let bob = db
            .create_node(&["Person"], serde_json::json!({"name": "Bob"}))
            .unwrap();

        let edge = db
            .create_edge(
                alice.id,
                bob.id,
                "KNOWS",
                serde_json::json!({"since": 2020}),
            )
            .unwrap();
        assert_eq!(edge.rel_type, "KNOWS");
        assert_eq!(edge.source, alice.id);
        assert_eq!(edge.target, bob.id);

        // Verify retrieval
        let got = db.get_node(alice.id).unwrap().unwrap();
        assert_eq!(got.id, alice.id);

        let neighbors = db.neighbors(alice.id, Some("KNOWS")).unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].1.id, bob.id);
    }

    #[test]
    fn test_cypher_query() {
        let (db, _dir) = setup();

        db.cypher("CREATE (n:Person {name: 'Alice', age: 30})")
            .unwrap();
        db.cypher("CREATE (n:Person {name: 'Bob', age: 25})")
            .unwrap();

        let result = db
            .cypher("MATCH (n:Person) WHERE n.age > 28 RETURN n.name AS name")
            .unwrap();
        assert_eq!(result.columns, vec!["name"]);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("name"),
            Some(&GraphValue::String("Alice".into()))
        );
    }

    #[test]
    fn test_cypher_with_params() {
        let (db, _dir) = setup();

        db.cypher("CREATE (n:Person {name: 'Alice', age: 30})")
            .unwrap();
        db.cypher("CREATE (n:Person {name: 'Bob', age: 25})")
            .unwrap();

        let result = db
            .cypher_with_params(
                "MATCH (n:Person) WHERE n.age > $min RETURN n.name AS name",
                serde_json::json!({"min": 28}),
            )
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("name"),
            Some(&GraphValue::String("Alice".into()))
        );
    }

    #[test]
    fn test_cypher_edge_traversal() {
        let (db, _dir) = setup();

        let alice = db
            .create_node(&["Person"], serde_json::json!({"name": "Alice"}))
            .unwrap();
        let bob = db
            .create_node(&["Person"], serde_json::json!({"name": "Bob"}))
            .unwrap();
        db.create_edge(
            alice.id,
            bob.id,
            "KNOWS",
            serde_json::json!({"since": 2020}),
        )
        .unwrap();

        let result = db
            .cypher(
                "MATCH (a:Person)-[:KNOWS]->(b:Person) WHERE a.name = 'Alice' RETURN b.name AS friend",
            )
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("friend"),
            Some(&GraphValue::String("Bob".into()))
        );
    }

    #[test]
    fn test_mixed_programmatic_and_cypher() {
        let (db, _dir) = setup();

        // Create programmatically
        let alice = db
            .create_node(&["Person"], serde_json::json!({"name": "Alice"}))
            .unwrap();
        let bob = db
            .create_node(&["Person"], serde_json::json!({"name": "Bob"}))
            .unwrap();
        db.create_edge(alice.id, bob.id, "KNOWS", serde_json::json!({}))
            .unwrap();

        // Query with Cypher
        let result = db
            .cypher("MATCH (p:Person) RETURN p.name AS name ORDER BY name")
            .unwrap();
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].get("name"),
            Some(&GraphValue::String("Alice".into()))
        );
        assert_eq!(
            result.rows[1].get("name"),
            Some(&GraphValue::String("Bob".into()))
        );
    }

    #[test]
    fn test_nodes_by_label() {
        let (db, _dir) = setup();

        db.create_node(&["Person"], serde_json::json!({"name": "Alice"}))
            .unwrap();
        db.create_node(&["Person"], serde_json::json!({"name": "Bob"}))
            .unwrap();
        db.create_node(&["Document"], serde_json::json!({"title": "Paper"}))
            .unwrap();

        let people = db.nodes_by_label("Person").unwrap();
        assert_eq!(people.len(), 2);

        let docs = db.nodes_by_label("Document").unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn test_delete_with_cypher() {
        let (db, _dir) = setup();

        db.cypher("CREATE (n:Person {name: 'Alice'})").unwrap();

        let before = db.cypher("MATCH (n:Person) RETURN n.name").unwrap();
        assert_eq!(before.rows.len(), 1);

        db.cypher("MATCH (n:Person) WHERE n.name = 'Alice' DETACH DELETE n")
            .unwrap();

        let after = db.cypher("MATCH (n:Person) RETURN n.name").unwrap();
        assert_eq!(after.rows.len(), 0);
    }

    #[test]
    fn test_set_with_cypher() {
        let (db, _dir) = setup();

        db.cypher("CREATE (n:Person {name: 'Alice', age: 30})")
            .unwrap();
        db.cypher("MATCH (n:Person) WHERE n.name = 'Alice' SET n.age = 31")
            .unwrap();

        let result = db
            .cypher("MATCH (n:Person) WHERE n.name = 'Alice' RETURN n.age AS age")
            .unwrap();
        assert_eq!(result.rows[0].get("age"), Some(&GraphValue::Integer(31)));
    }

    #[test]
    fn test_prd_example_query() {
        let (db, _dir) = setup();

        // Build a document graph
        let ai_overview = db
            .create_node(
                &["Document"],
                serde_json::json!({"title": "AI Overview", "topic": "AI"}),
            )
            .unwrap();
        let deep_learning = db
            .create_node(
                &["Document"],
                serde_json::json!({"title": "Deep Learning", "topic": "AI"}),
            )
            .unwrap();
        let _cooking = db
            .create_node(
                &["Document"],
                serde_json::json!({"title": "Cooking Guide", "topic": "Food"}),
            )
            .unwrap();

        db.create_edge(
            ai_overview.id,
            deep_learning.id,
            "REFERENCES",
            serde_json::json!({}),
        )
        .unwrap();

        // PRD example: graph traversal + filter
        let result = db
            .cypher(
                "MATCH (doc:Document)-[:REFERENCES]->(ref:Document) \
                 WHERE doc.topic = 'AI' \
                 RETURN ref.title AS title \
                 ORDER BY title \
                 LIMIT 5",
            )
            .unwrap();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("title"),
            Some(&GraphValue::String("Deep Learning".into()))
        );
    }

    #[test]
    fn test_cypher_parse_error() {
        let (db, _dir) = setup();
        let result = db.cypher("INVALID SYNTAX HERE");
        assert!(result.is_err());
    }

    #[test]
    fn test_vector_similarity_query() {
        let (db, _dir) = setup();

        // Create documents with vectors
        db.create_node_with_vector(
            &["Document"],
            serde_json::json!({"title": "AI Paper"}),
            vec![1.0, 0.0, 0.0],
        )
        .unwrap();
        db.create_node_with_vector(
            &["Document"],
            serde_json::json!({"title": "ML Paper"}),
            vec![0.9, 0.1, 0.0],
        )
        .unwrap();
        db.create_node_with_vector(
            &["Document"],
            serde_json::json!({"title": "Cooking Guide"}),
            vec![0.0, 0.0, 1.0],
        )
        .unwrap();

        // Graph + vector combined query
        let result = db
            .cypher_with_params(
                "MATCH (n:Document) \
                 WHERE vector_similarity(n.embedding, $query) > 0.5 \
                 RETURN n.title AS title, vector_similarity(n.embedding, $query) AS score \
                 ORDER BY score DESC",
                serde_json::json!({"query": [1.0, 0.0, 0.0]}),
            )
            .unwrap();

        // Should find AI Paper and ML Paper (both similar), not Cooking Guide (orthogonal)
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            result.rows[0].get("title"),
            Some(&GraphValue::String("AI Paper".into()))
        );
    }

    // ─── Shared storage tests ────────────────────────────────────

    #[test]
    fn test_shared_storage_vector_insert_and_query() {
        let (db, _dir) = setup();

        let id1 = uuid::Uuid::new_v4();
        let id2 = uuid::Uuid::new_v4();
        let id3 = uuid::Uuid::new_v4();

        db.insert_vector(id1, &[1.0, 0.0, 0.0], serde_json::json!({"title": "AI"}))
            .unwrap();
        db.insert_vector(id2, &[0.0, 1.0, 0.0], serde_json::json!({"title": "Bio"}))
            .unwrap();
        db.insert_vector(id3, &[0.0, 0.0, 1.0], serde_json::json!({"title": "Chem"}))
            .unwrap();

        // Retrieve vector
        let vec = db.get_vector(id1).unwrap().unwrap();
        assert_eq!(vec, vec![1.0, 0.0, 0.0]);

        // kNN query
        let results = db.query_vectors(&[1.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, id1); // Exact match first
        assert!(results[0].1 > 0.99);
    }

    #[test]
    fn test_shared_storage_graph_and_vector_coexist() {
        let (db, _dir) = setup();

        // Create graph data
        let alice = db
            .create_node(&["Person"], serde_json::json!({"name": "Alice"}))
            .unwrap();
        let doc = db
            .create_node_with_vector(
                &["Document"],
                serde_json::json!({"title": "Paper"}),
                vec![1.0, 0.0, 0.0],
            )
            .unwrap();
        db.create_edge(alice.id, doc.id, "AUTHORED", serde_json::json!({}))
            .unwrap();

        // Also insert standalone vectors
        let vec_id = uuid::Uuid::new_v4();
        db.insert_vector(
            vec_id,
            &[0.5, 0.5, 0.0],
            serde_json::json!({"type": "standalone"}),
        )
        .unwrap();

        // Graph queries still work
        let result = db
            .cypher("MATCH (p:Person)-[:AUTHORED]->(d:Document) RETURN d.title AS title")
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("title"),
            Some(&GraphValue::String("Paper".into()))
        );

        // Vector queries work
        let results = db.query_vectors(&[1.0, 0.0, 0.0], 5).unwrap();
        assert!(!results.is_empty());

        // Standalone vector is retrievable
        let vec = db.get_vector(vec_id).unwrap().unwrap();
        assert_eq!(vec, vec![0.5, 0.5, 0.0]);
    }

    #[test]
    fn test_single_directory_single_db() {
        let dir = TempDir::new().unwrap();

        // Open, do work, close
        {
            let db = GraphIndex::open(dir.path()).unwrap();
            db.cypher("CREATE (n:Person {name: 'Alice'})").unwrap();
            let id = uuid::Uuid::new_v4();
            db.insert_vector(id, &[1.0, 0.0], serde_json::json!({"test": true}))
                .unwrap();
        }

        // Re-open same path — data should persist
        {
            let db = GraphIndex::open(dir.path()).unwrap();
            let result = db.cypher("MATCH (n:Person) RETURN n.name AS name").unwrap();
            assert_eq!(result.rows.len(), 1);
            assert_eq!(
                result.rows[0].get("name"),
                Some(&GraphValue::String("Alice".into()))
            );
        }
    }

    #[test]
    fn test_graph_stats() {
        let (db, _dir) = setup();

        let alice = db
            .create_node(&["Person"], serde_json::json!({"name": "Alice"}))
            .unwrap();
        let bob = db
            .create_node(&["Person"], serde_json::json!({"name": "Bob"}))
            .unwrap();
        db.create_node_with_vector(
            &["Document"],
            serde_json::json!({"title": "Paper"}),
            vec![1.0, 0.0, 0.0],
        )
        .unwrap();
        db.create_edge(alice.id, bob.id, "KNOWS", serde_json::json!({}))
            .unwrap();

        let stats = db.graph_stats().unwrap();
        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.edge_count, 1);
        assert!(stats.labels.contains(&"Person".to_string()));
        assert!(stats.labels.contains(&"Document".to_string()));
        assert_eq!(stats.relationship_types, vec!["KNOWS".to_string()]);
        assert!(stats.has_vectors);
    }

    #[test]
    fn test_property_index_speeds_up_merge() {
        let (db, _dir) = setup();

        // Create index on Function.name
        db.create_property_index("Function", "name").unwrap();

        // Create 100 nodes
        for i in 0..100 {
            db.cypher(&format!("CREATE (n:Function {{name: 'fn_{}'}})", i))
                .unwrap();
        }

        // MERGE should use the index for fast lookup
        let start = std::time::Instant::now();
        for i in 0..100 {
            db.cypher(&format!("MERGE (n:Function {{name: 'fn_{}'}})", i))
                .unwrap();
        }
        let merge_time = start.elapsed();

        // Should still have exactly 100 nodes (MERGE found all, didn't create new)
        let result = db
            .cypher("MATCH (n:Function) RETURN count(*) AS c")
            .unwrap();
        assert_eq!(result.rows[0].get("c"), Some(&GraphValue::Integer(100)));

        // With index, 100 MERGEs should be well under 1 second
        assert!(
            merge_time.as_millis() < 2000,
            "MERGE with index too slow: {}ms",
            merge_time.as_millis()
        );
    }

    #[test]
    fn test_property_index_used_in_match() {
        let (db, _dir) = setup();

        // Create many nodes
        for i in 0..500 {
            db.cypher(&format!("CREATE (n:Item {{key: 'item_{}'}})", i))
                .unwrap();
        }

        // Create index
        db.create_property_index("Item", "key").unwrap();

        // MATCH with inline properties should use index
        let start = std::time::Instant::now();
        let result = db
            .cypher("MATCH (n:Item {key: 'item_250'}) RETURN n.key AS k")
            .unwrap();
        let query_time = start.elapsed();

        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("k"),
            Some(&GraphValue::String("item_250".into()))
        );

        // Indexed lookup should be fast
        assert!(
            query_time.as_millis() < 50,
            "Indexed MATCH too slow: {}ms",
            query_time.as_millis()
        );
    }
}
