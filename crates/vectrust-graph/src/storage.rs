// Copyright 2024-2026 Andrey Vasilevsky <anvanster@gmail.com>
// SPDX-License-Identifier: Apache-2.0

use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch, DB};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use vectrust_core::{
    EdgeRecord, GraphEdge, GraphNode, GraphValue, HnswConfig, NodeRecord, Result, VectraError,
};
use vectrust_index::HnswIndex;

// Column family names
const CF_NODES: &str = "graph_nodes";
const CF_NODE_PROPS: &str = "graph_node_props";
const CF_EDGES: &str = "graph_edges";
const CF_EDGE_PROPS: &str = "graph_edge_props";
const CF_ADJ_OUT: &str = "graph_adj_out";
const CF_ADJ_IN: &str = "graph_adj_in";
const CF_LABEL_IDX: &str = "graph_label_idx";
const CF_RELTYPE_IDX: &str = "graph_reltype_idx";
const CF_NODE_VECTORS: &str = "graph_node_vectors";
const CF_PROP_IDX: &str = "graph_prop_idx";

const ALL_CFS: &[&str] = &[
    CF_NODES,
    CF_NODE_PROPS,
    CF_EDGES,
    CF_EDGE_PROPS,
    CF_ADJ_OUT,
    CF_ADJ_IN,
    CF_LABEL_IDX,
    CF_RELTYPE_IDX,
    CF_NODE_VECTORS,
    CF_PROP_IDX,
];

/// All column families needed for a shared graph+vector database.
pub const SHARED_CFS: &[&str] = &[
    CF_NODES,
    CF_NODE_PROPS,
    CF_EDGES,
    CF_EDGE_PROPS,
    CF_ADJ_OUT,
    CF_ADJ_IN,
    CF_LABEL_IDX,
    CF_RELTYPE_IDX,
    CF_NODE_VECTORS,
    CF_PROP_IDX,
    // Vector storage CFs (compatible with OptimizedStorage)
    "metadata",
    "vector_index",
];

/// Internal DB handle — owned or shared.
enum DbHandle {
    Owned(DB),
    Shared(Arc<DB>),
}

impl DbHandle {
    fn db(&self) -> &DB {
        match self {
            DbHandle::Owned(db) => db,
            DbHandle::Shared(db) => db,
        }
    }
}

/// Graph storage backed by RocksDB column families.
pub struct GraphStorage {
    handle: DbHandle,
    #[allow(dead_code)]
    path: PathBuf,
    /// Cached HNSW index for accelerated kNN. Built lazily on first query.
    hnsw_cache: RwLock<Option<HnswIndex>>,
    /// Set of (label, property) pairs that have property indexes.
    indexed_properties: RwLock<std::collections::HashSet<(String, String)>>,
}

impl GraphStorage {
    /// Open or create graph-only storage at the given path.
    /// Uses a dedicated `graph/` subdirectory with graph CFs only.
    pub fn open(path: &Path) -> Result<Self> {
        let db_path = path.join("graph");
        std::fs::create_dir_all(&db_path)?;

        let db = Self::open_db(&db_path, ALL_CFS)?;

        Ok(Self {
            handle: DbHandle::Owned(db),
            path: db_path,
            hnsw_cache: RwLock::new(None),
            indexed_properties: RwLock::new(std::collections::HashSet::new()),
        })
    }

    /// Open shared storage with a pre-opened DB that has all CFs.
    /// Used by GraphIndex for unified graph+vector storage.
    pub fn open_shared(db: Arc<DB>, path: PathBuf) -> Self {
        Self {
            handle: DbHandle::Shared(db),
            path,
            hnsw_cache: RwLock::new(None),
            indexed_properties: RwLock::new(std::collections::HashSet::new()),
        }
    }

    /// Open a shared graph+vector database at the given path.
    /// Returns the Arc<DB> so the caller can also use it for vector operations.
    pub fn open_shared_db(path: &Path) -> Result<(Self, Arc<DB>)> {
        let db_path = path.join("db");
        std::fs::create_dir_all(&db_path)?;

        let db = Arc::new(Self::open_db(&db_path, SHARED_CFS)?);
        let storage = Self::open_shared(Arc::clone(&db), db_path);
        Ok((storage, db))
    }

    fn open_db(db_path: &Path, cf_names: &[&str]) -> Result<DB> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_max_write_buffer_number(3);
        db_opts.set_write_buffer_size(32 * 1024 * 1024);
        db_opts.set_level_compaction_dynamic_level_bytes(true);
        db_opts.set_max_background_jobs(2);

        let cf_descriptors: Vec<ColumnFamilyDescriptor> = cf_names
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
            .collect();

        DB::open_cf_descriptors(&db_opts, db_path, cf_descriptors).map_err(|e| {
            VectraError::StorageError {
                message: e.to_string(),
            }
        })
    }

    fn db(&self) -> &DB {
        self.handle.db()
    }

    // ─── Node operations ─────────────────────────────────────────

    /// Create a node with labels and properties. Returns the node ID.
    pub fn create_node(
        &self,
        labels: &[String],
        properties: HashMap<String, GraphValue>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let record = NodeRecord {
            id,
            labels: labels.to_vec(),
            has_vector: false,
        };

        // Write node record
        let cf_nodes = self.cf(CF_NODES)?;
        let key = node_key(id);
        let value = bincode::serialize(&record).map_err(|e| VectraError::StorageError {
            message: e.to_string(),
        })?;
        self.db().put_cf(&cf_nodes, &key, &value)?;

        // Write properties
        if !properties.is_empty() {
            let cf_props = self.cf(CF_NODE_PROPS)?;
            let props_json = serde_json::to_vec(&properties)?;
            self.db().put_cf(&cf_props, &key, &props_json)?;
        }

        // Update label index
        let cf_label = self.cf(CF_LABEL_IDX)?;
        for label in labels {
            let idx_key = label_index_key(label, id);
            self.db().put_cf(&cf_label, &idx_key, [])?;
        }

        // Update property indexes
        self.update_property_indexes(id, labels, &properties)?;

        Ok(id)
    }

    /// Create a node with labels, properties, and an embedding vector.
    pub fn create_node_with_vector(
        &self,
        labels: &[String],
        properties: HashMap<String, GraphValue>,
        vector: Vec<f32>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let record = NodeRecord {
            id,
            labels: labels.to_vec(),
            has_vector: true,
        };

        // Write node record
        let cf_nodes = self.cf(CF_NODES)?;
        let key = node_key(id);
        let value = bincode::serialize(&record).map_err(|e| VectraError::StorageError {
            message: e.to_string(),
        })?;
        self.db().put_cf(&cf_nodes, &key, &value)?;

        // Write properties
        if !properties.is_empty() {
            let cf_props = self.cf(CF_NODE_PROPS)?;
            let props_json = serde_json::to_vec(&properties)?;
            self.db().put_cf(&cf_props, &key, &props_json)?;
        }

        // Write vector
        let cf_vectors = self.cf(CF_NODE_VECTORS)?;
        let vector_bytes = vector_to_bytes(&vector);
        self.db().put_cf(&cf_vectors, &key, &vector_bytes)?;

        // Update label index
        let cf_label = self.cf(CF_LABEL_IDX)?;
        for label in labels {
            let idx_key = label_index_key(label, id);
            self.db().put_cf(&cf_label, &idx_key, [])?;
        }

        self.invalidate_hnsw_cache();
        self.update_property_indexes(id, labels, &properties)?;
        Ok(id)
    }

    /// Batch-create multiple nodes in a single RocksDB WriteBatch.
    /// Returns the IDs of the created nodes.
    pub fn create_nodes_batch(
        &self,
        nodes: &[(Vec<String>, HashMap<String, GraphValue>)],
    ) -> Result<Vec<Uuid>> {
        let mut batch = WriteBatch::default();
        let cf_nodes = self.cf(CF_NODES)?;
        let cf_props = self.cf(CF_NODE_PROPS)?;
        let cf_label = self.cf(CF_LABEL_IDX)?;

        let mut ids = Vec::with_capacity(nodes.len());

        for (labels, properties) in nodes {
            let id = Uuid::new_v4();
            let record = NodeRecord {
                id,
                labels: labels.clone(),
                has_vector: false,
            };

            let key = node_key(id);
            let value = bincode::serialize(&record).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            batch.put_cf(&cf_nodes, &key, &value);

            if !properties.is_empty() {
                let props_json = serde_json::to_vec(properties)?;
                batch.put_cf(&cf_props, &key, &props_json);
            }

            for label in labels {
                batch.put_cf(&cf_label, label_index_key(label, id), []);
            }

            ids.push(id);
        }

        self.db()
            .write(batch)
            .map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;

        Ok(ids)
    }

    /// Batch-create multiple edges in a single RocksDB WriteBatch.
    /// Each edge is (source_id, target_id, rel_type, properties).
    /// Returns the IDs of the created edges.
    pub fn create_edges_batch(
        &self,
        edges: &[(Uuid, Uuid, String, HashMap<String, GraphValue>)],
    ) -> Result<Vec<Uuid>> {
        let mut batch = WriteBatch::default();
        let cf_edges = self.cf(CF_EDGES)?;
        let cf_props = self.cf(CF_EDGE_PROPS)?;
        let cf_out = self.cf(CF_ADJ_OUT)?;
        let cf_in = self.cf(CF_ADJ_IN)?;
        let cf_reltype = self.cf(CF_RELTYPE_IDX)?;

        let mut ids = Vec::with_capacity(edges.len());

        for (source, target, rel_type, properties) in edges {
            let id = Uuid::new_v4();
            let record = EdgeRecord {
                id,
                source: *source,
                target: *target,
                rel_type: rel_type.clone(),
            };

            let key = edge_key(id);
            let value = bincode::serialize(&record).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            batch.put_cf(&cf_edges, &key, &value);

            if !properties.is_empty() {
                let props_json = serde_json::to_vec(properties)?;
                batch.put_cf(&cf_props, &key, &props_json);
            }

            batch.put_cf(&cf_out, adj_out_key(*source, id), target.as_bytes());
            batch.put_cf(&cf_in, adj_in_key(*target, id), source.as_bytes());
            batch.put_cf(&cf_reltype, reltype_index_key(rel_type, id), []);

            ids.push(id);
        }

        self.db()
            .write(batch)
            .map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;

        Ok(ids)
    }

    /// Get the vector associated with a node, if any.
    pub fn get_node_vector(&self, id: Uuid) -> Result<Option<Vec<f32>>> {
        let cf_vectors = self.cf(CF_NODE_VECTORS)?;
        match self.db().get_cf(&cf_vectors, node_key(id))? {
            Some(bytes) => Ok(Some(bytes_to_vector(&bytes))),
            None => Ok(None),
        }
    }

    /// Set or replace the vector on an existing node.
    pub fn set_node_vector(&self, id: Uuid, vector: Vec<f32>) -> Result<()> {
        let cf_vectors = self.cf(CF_NODE_VECTORS)?;
        let vector_bytes = vector_to_bytes(&vector);
        self.db().put_cf(&cf_vectors, node_key(id), &vector_bytes)?;
        self.invalidate_hnsw_cache();
        Ok(())
    }

    /// Scan all nodes that have vectors, returning (node_id, vector) pairs.
    /// Used for brute-force kNN when no HNSW index is available.
    pub fn all_node_vectors(&self) -> Result<Vec<(Uuid, Vec<f32>)>> {
        let cf = self.cf(CF_NODE_VECTORS)?;
        let iter = self.db().iterator_cf(&cf, IteratorMode::Start);
        let mut results = Vec::new();
        for item in iter {
            let (key, value) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            if let Some(uuid_str) = key_str.strip_prefix("n:") {
                let uuid = Uuid::parse_str(uuid_str).map_err(|e| VectraError::StorageError {
                    message: e.to_string(),
                })?;
                let vector = bytes_to_vector(&value);
                results.push((uuid, vector));
            }
        }
        Ok(results)
    }

    /// HNSW-accelerated kNN search over node vectors.
    /// Lazily builds the HNSW index on first call, then caches it.
    /// Returns (node_id, similarity_score) pairs sorted by descending similarity.
    pub fn nearest_vectors(&self, query: &[f32], k: usize) -> Result<Vec<(Uuid, f32)>> {
        // Try to use cached HNSW index
        {
            let cache = self.hnsw_cache.read().map_err(|e| VectraError::Lock {
                message: e.to_string(),
            })?;
            if let Some(ref index) = *cache {
                if !index.is_empty() {
                    let results = index.search(query, k)?;
                    // Convert from distance to similarity (for cosine: sim = 1.0 - dist)
                    return Ok(results
                        .into_iter()
                        .map(|(id, dist)| (id, 1.0 - dist))
                        .collect());
                }
            }
        }

        // Build index from stored vectors
        self.rebuild_hnsw_index()?;

        // Search again with the newly built index
        let cache = self.hnsw_cache.read().map_err(|e| VectraError::Lock {
            message: e.to_string(),
        })?;
        if let Some(ref index) = *cache {
            let results = index.search(query, k)?;
            Ok(results
                .into_iter()
                .map(|(id, dist)| (id, 1.0 - dist))
                .collect())
        } else {
            // No vectors at all
            Ok(Vec::new())
        }
    }

    /// Rebuild the HNSW index from all stored node vectors.
    fn rebuild_hnsw_index(&self) -> Result<()> {
        let all_vectors = self.all_node_vectors()?;
        if all_vectors.is_empty() {
            return Ok(());
        }

        let config = HnswConfig::default();
        let mut index = HnswIndex::new(config)?;

        for (id, vector) in &all_vectors {
            index.insert(*id, vector)?;
        }

        let mut cache = self.hnsw_cache.write().map_err(|e| VectraError::Lock {
            message: e.to_string(),
        })?;
        *cache = Some(index);

        Ok(())
    }

    /// Invalidate the cached HNSW index (call after inserting/removing vectors).
    fn invalidate_hnsw_cache(&self) {
        if let Ok(mut cache) = self.hnsw_cache.write() {
            *cache = None;
        }
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: Uuid) -> Result<Option<GraphNode>> {
        let cf_nodes = self.cf(CF_NODES)?;
        let key = node_key(id);

        let record_bytes = match self.db().get_cf(&cf_nodes, &key)? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let record: NodeRecord =
            bincode::deserialize(&record_bytes).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;

        let properties = self.get_node_properties(id)?;

        Ok(Some(GraphNode {
            id: record.id,
            labels: record.labels,
            properties,
        }))
    }

    /// Delete a node and all its edges.
    pub fn delete_node(&self, id: Uuid, detach: bool) -> Result<()> {
        // Check if node has edges
        let out_edges = self.get_outgoing_edges(id)?;
        let in_edges = self.get_incoming_edges(id)?;

        if (!out_edges.is_empty() || !in_edges.is_empty()) && !detach {
            return Err(VectraError::Graph {
                message: format!(
                    "Cannot delete node {} with existing relationships. Use DETACH DELETE.",
                    id
                ),
            });
        }

        // Delete all connected edges
        for edge_id in out_edges.iter().chain(in_edges.iter()) {
            self.delete_edge_internal(*edge_id)?;
        }

        // Delete node record
        let key = node_key(id);
        if let Some(node) = self.get_node(id)? {
            // Remove from label index
            let cf_label = self.cf(CF_LABEL_IDX)?;
            for label in &node.labels {
                self.db().delete_cf(&cf_label, label_index_key(label, id))?;
            }
        }

        self.db().delete_cf(&self.cf(CF_NODES)?, &key)?;
        self.db().delete_cf(&self.cf(CF_NODE_PROPS)?, &key)?;

        Ok(())
    }

    /// Set a property on a node.
    pub fn set_node_property(&self, id: Uuid, key: &str, value: GraphValue) -> Result<()> {
        let mut props = self.get_node_properties(id)?;
        props.insert(key.to_string(), value.clone());
        let cf_props = self.cf(CF_NODE_PROPS)?;
        let props_json = serde_json::to_vec(&props)?;
        self.db().put_cf(&cf_props, node_key(id), &props_json)?;

        // Update property index if applicable
        if let Some(node) = self.get_node(id)? {
            let cf = self.cf(CF_PROP_IDX)?;
            if let Ok(indexed) = self.indexed_properties.read() {
                for label in &node.labels {
                    if indexed.contains(&(label.clone(), key.to_string())) {
                        let idx_key = prop_index_key(label, key, &value, id);
                        self.db().put_cf(&cf, &idx_key, [])?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove a property from a node.
    pub fn remove_node_property(&self, id: Uuid, key: &str) -> Result<()> {
        let mut props = self.get_node_properties(id)?;
        props.remove(key);
        let cf_props = self.cf(CF_NODE_PROPS)?;
        let props_json = serde_json::to_vec(&props)?;
        self.db().put_cf(&cf_props, node_key(id), &props_json)?;
        Ok(())
    }

    fn get_node_properties(&self, id: Uuid) -> Result<HashMap<String, GraphValue>> {
        let cf_props = self.cf(CF_NODE_PROPS)?;
        match self.db().get_cf(&cf_props, node_key(id))? {
            Some(bytes) => {
                let props: HashMap<String, GraphValue> = serde_json::from_slice(&bytes)?;
                Ok(props)
            }
            None => Ok(HashMap::new()),
        }
    }

    // ─── Edge operations ─────────────────────────────────────────

    /// Create an edge between two nodes. Returns the edge ID.
    pub fn create_edge(
        &self,
        source: Uuid,
        target: Uuid,
        rel_type: &str,
        properties: HashMap<String, GraphValue>,
    ) -> Result<Uuid> {
        // Verify both nodes exist
        if self.get_node(source)?.is_none() {
            return Err(VectraError::NodeNotFound {
                id: source.to_string(),
            });
        }
        if self.get_node(target)?.is_none() {
            return Err(VectraError::NodeNotFound {
                id: target.to_string(),
            });
        }

        let id = Uuid::new_v4();
        let record = EdgeRecord {
            id,
            source,
            target,
            rel_type: rel_type.to_string(),
        };

        // Write edge record
        let cf_edges = self.cf(CF_EDGES)?;
        let key = edge_key(id);
        let value = bincode::serialize(&record).map_err(|e| VectraError::StorageError {
            message: e.to_string(),
        })?;
        self.db().put_cf(&cf_edges, &key, &value)?;

        // Write properties
        if !properties.is_empty() {
            let cf_props = self.cf(CF_EDGE_PROPS)?;
            let props_json = serde_json::to_vec(&properties)?;
            self.db().put_cf(&cf_props, &key, &props_json)?;
        }

        // Update adjacency lists
        let cf_out = self.cf(CF_ADJ_OUT)?;
        self.db()
            .put_cf(&cf_out, adj_out_key(source, id), target.as_bytes())?;

        let cf_in = self.cf(CF_ADJ_IN)?;
        self.db()
            .put_cf(&cf_in, adj_in_key(target, id), source.as_bytes())?;

        // Update relationship type index
        let cf_reltype = self.cf(CF_RELTYPE_IDX)?;
        self.db()
            .put_cf(&cf_reltype, reltype_index_key(rel_type, id), [])?;

        Ok(id)
    }

    /// Get an edge by ID.
    pub fn get_edge(&self, id: Uuid) -> Result<Option<GraphEdge>> {
        let cf_edges = self.cf(CF_EDGES)?;
        let key = edge_key(id);

        let record_bytes = match self.db().get_cf(&cf_edges, &key)? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let record: EdgeRecord =
            bincode::deserialize(&record_bytes).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;

        let properties = self.get_edge_properties(id)?;

        Ok(Some(GraphEdge {
            id: record.id,
            source: record.source,
            target: record.target,
            rel_type: record.rel_type,
            properties,
        }))
    }

    /// Delete an edge by ID.
    pub fn delete_edge(&self, id: Uuid) -> Result<()> {
        self.delete_edge_internal(id)
    }

    fn delete_edge_internal(&self, id: Uuid) -> Result<()> {
        let cf_edges = self.cf(CF_EDGES)?;
        let key = edge_key(id);

        if let Some(bytes) = self.db().get_cf(&cf_edges, &key)? {
            let record: EdgeRecord =
                bincode::deserialize(&bytes).map_err(|e| VectraError::StorageError {
                    message: e.to_string(),
                })?;

            // Remove adjacency entries
            self.db()
                .delete_cf(&self.cf(CF_ADJ_OUT)?, adj_out_key(record.source, id))?;
            self.db()
                .delete_cf(&self.cf(CF_ADJ_IN)?, adj_in_key(record.target, id))?;

            // Remove from reltype index
            self.db().delete_cf(
                &self.cf(CF_RELTYPE_IDX)?,
                reltype_index_key(&record.rel_type, id),
            )?;

            // Remove edge record and properties
            self.db().delete_cf(&cf_edges, &key)?;
            self.db().delete_cf(&self.cf(CF_EDGE_PROPS)?, &key)?;
        }

        Ok(())
    }

    fn get_edge_properties(&self, id: Uuid) -> Result<HashMap<String, GraphValue>> {
        let cf_props = self.cf(CF_EDGE_PROPS)?;
        match self.db().get_cf(&cf_props, edge_key(id))? {
            Some(bytes) => {
                let props: HashMap<String, GraphValue> = serde_json::from_slice(&bytes)?;
                Ok(props)
            }
            None => Ok(HashMap::new()),
        }
    }

    // ─── Query operations (used by executor) ─────────────────────

    /// Get all node IDs with a given label.
    pub fn nodes_by_label(&self, label: &str) -> Result<Vec<Uuid>> {
        let cf = self.cf(CF_LABEL_IDX)?;
        let prefix = format!("li:{}:", label);
        let iter = self.db().prefix_iterator_cf(&cf, prefix.as_bytes());

        let mut ids = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            // Key format: "li:{label}:{uuid}"
            if !key_str.starts_with(&prefix) {
                break; // Past our prefix
            }
            if let Some(uuid_str) = key_str.strip_prefix(&prefix) {
                let uuid = Uuid::parse_str(uuid_str).map_err(|e| VectraError::StorageError {
                    message: e.to_string(),
                })?;
                ids.push(uuid);
            }
        }

        Ok(ids)
    }

    /// Get all node IDs (full scan).
    pub fn all_nodes(&self) -> Result<Vec<Uuid>> {
        let cf = self.cf(CF_NODES)?;
        let iter = self.db().iterator_cf(&cf, IteratorMode::Start);

        let mut ids = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            if let Some(uuid_str) = key_str.strip_prefix("n:") {
                let uuid = Uuid::parse_str(uuid_str).map_err(|e| VectraError::StorageError {
                    message: e.to_string(),
                })?;
                ids.push(uuid);
            }
        }

        Ok(ids)
    }

    /// Get outgoing edge IDs from a node.
    fn get_outgoing_edges(&self, node_id: Uuid) -> Result<Vec<Uuid>> {
        let cf = self.cf(CF_ADJ_OUT)?;
        let prefix = format!("ao:{}:", node_id);
        self.scan_edge_ids_by_prefix(&cf, &prefix)
    }

    /// Get incoming edge IDs to a node.
    fn get_incoming_edges(&self, node_id: Uuid) -> Result<Vec<Uuid>> {
        let cf = self.cf(CF_ADJ_IN)?;
        let prefix = format!("ai:{}:", node_id);
        self.scan_edge_ids_by_prefix(&cf, &prefix)
    }

    /// Expand outgoing edges from a node, optionally filtered by relationship type.
    /// Returns (edge, target_node) pairs.
    pub fn expand_out(
        &self,
        node_id: Uuid,
        rel_types: &[String],
    ) -> Result<Vec<(GraphEdge, GraphNode)>> {
        let edge_ids = self.get_outgoing_edges(node_id)?;
        let mut results = Vec::new();

        for edge_id in edge_ids {
            if let Some(edge) = self.get_edge(edge_id)? {
                if !rel_types.is_empty() && !rel_types.contains(&edge.rel_type) {
                    continue;
                }
                if let Some(target) = self.get_node(edge.target)? {
                    results.push((edge, target));
                }
            }
        }

        Ok(results)
    }

    /// Expand incoming edges to a node, optionally filtered by relationship type.
    /// Returns (edge, source_node) pairs.
    pub fn expand_in(
        &self,
        node_id: Uuid,
        rel_types: &[String],
    ) -> Result<Vec<(GraphEdge, GraphNode)>> {
        let edge_ids = self.get_incoming_edges(node_id)?;
        let mut results = Vec::new();

        for edge_id in edge_ids {
            if let Some(edge) = self.get_edge(edge_id)? {
                if !rel_types.is_empty() && !rel_types.contains(&edge.rel_type) {
                    continue;
                }
                if let Some(source) = self.get_node(edge.source)? {
                    results.push((edge, source));
                }
            }
        }

        Ok(results)
    }

    /// Expand in both directions.
    pub fn expand_both(
        &self,
        node_id: Uuid,
        rel_types: &[String],
    ) -> Result<Vec<(GraphEdge, GraphNode)>> {
        let mut results = self.expand_out(node_id, rel_types)?;
        results.extend(self.expand_in(node_id, rel_types)?);
        Ok(results)
    }

    // ─── Helpers ─────────────────────────────────────────────────

    fn cf(&self, name: &str) -> Result<std::sync::Arc<rocksdb::BoundColumnFamily>> {
        self.db()
            .cf_handle(name)
            .ok_or_else(|| VectraError::StorageError {
                message: format!("column family '{}' not found", name),
            })
    }

    /// Count all edges.
    pub fn edge_count(&self) -> Result<usize> {
        let cf = self.cf(CF_EDGES)?;
        let iter = self.db().iterator_cf(&cf, IteratorMode::Start);
        Ok(iter.count())
    }

    /// Get all distinct labels.
    pub fn all_labels(&self) -> Result<Vec<String>> {
        let cf = self.cf(CF_LABEL_IDX)?;
        let iter = self.db().iterator_cf(&cf, IteratorMode::Start);
        let mut labels = std::collections::HashSet::new();
        for item in iter {
            let (key, _) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            // Key format: "li:{label}:{node_uuid}"
            if let Some(rest) = key_str.strip_prefix("li:") {
                if let Some(label) = rest.split(':').next() {
                    labels.insert(label.to_string());
                }
            }
        }
        let mut result: Vec<String> = labels.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Get all distinct relationship types.
    pub fn all_relationship_types(&self) -> Result<Vec<String>> {
        let cf = self.cf(CF_RELTYPE_IDX)?;
        let iter = self.db().iterator_cf(&cf, IteratorMode::Start);
        let mut types = std::collections::HashSet::new();
        for item in iter {
            let (key, _) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            // Key format: "ri:{type}:{edge_uuid}"
            if let Some(rest) = key_str.strip_prefix("ri:") {
                if let Some(rel_type) = rest.split(':').next() {
                    types.insert(rel_type.to_string());
                }
            }
        }
        let mut result: Vec<String> = types.into_iter().collect();
        result.sort();
        Ok(result)
    }

    /// Check if any nodes have vectors.
    pub fn has_vectors(&self) -> Result<bool> {
        let cf = self.cf(CF_NODE_VECTORS)?;
        let mut iter = self.db().iterator_cf(&cf, IteratorMode::Start);
        Ok(iter.next().is_some())
    }

    // ─── Property indexes ─────────────────────────────────────────

    /// Create a property index for fast lookups on (label, property) pairs.
    /// Scans all existing nodes with the label and indexes the property values.
    pub fn create_property_index(&self, label: &str, property: &str) -> Result<()> {
        let cf = self.cf(CF_PROP_IDX)?;

        // Scan all nodes with this label
        let node_ids = self.nodes_by_label(label)?;
        for id in node_ids {
            if let Some(node) = self.get_node(id)? {
                if let Some(value) = node.properties.get(property) {
                    let key = prop_index_key(label, property, value, id);
                    self.db().put_cf(&cf, &key, [])?;
                }
            }
        }

        // Register the index
        if let Ok(mut indexed) = self.indexed_properties.write() {
            indexed.insert((label.to_string(), property.to_string()));
        }

        Ok(())
    }

    /// Check if a property index exists for (label, property).
    pub fn has_property_index(&self, label: &str, property: &str) -> bool {
        self.indexed_properties
            .read()
            .map(|s| s.contains(&(label.to_string(), property.to_string())))
            .unwrap_or(false)
    }

    /// Look up nodes by indexed property value. Returns node IDs.
    /// Falls back to full scan if no index exists.
    pub fn nodes_by_property(
        &self,
        label: &str,
        property: &str,
        value: &GraphValue,
    ) -> Result<Option<Vec<Uuid>>> {
        if !self.has_property_index(label, property) {
            return Ok(None); // No index — caller should fall back to scan
        }

        let cf = self.cf(CF_PROP_IDX)?;
        let prefix = prop_index_prefix(label, property, value);
        let iter = self.db().prefix_iterator_cf(&cf, prefix.as_bytes());

        let mut ids = Vec::new();
        for item in iter {
            let (key, _) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            if !key_str.starts_with(&prefix) {
                break;
            }
            // Key: pi:{label}:{property}:{value_hash}:{uuid}
            if let Some(uuid_str) = key_str.rsplit(':').next() {
                if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                    ids.push(uuid);
                }
            }
        }

        Ok(Some(ids))
    }

    /// Update property indexes for a node after creation or property change.
    fn update_property_indexes(
        &self,
        node_id: Uuid,
        labels: &[String],
        properties: &HashMap<String, GraphValue>,
    ) -> Result<()> {
        let indexed = match self.indexed_properties.read() {
            Ok(s) => s.clone(),
            Err(_) => return Ok(()),
        };

        if indexed.is_empty() {
            return Ok(());
        }

        let cf = self.cf(CF_PROP_IDX)?;
        for (idx_label, idx_prop) in &indexed {
            if labels.contains(idx_label) {
                if let Some(value) = properties.get(idx_prop) {
                    let key = prop_index_key(idx_label, idx_prop, value, node_id);
                    self.db().put_cf(&cf, &key, [])?;
                }
            }
        }

        Ok(())
    }

    fn scan_edge_ids_by_prefix(
        &self,
        cf: &std::sync::Arc<rocksdb::BoundColumnFamily>,
        prefix: &str,
    ) -> Result<Vec<Uuid>> {
        let iter = self.db().prefix_iterator_cf(cf, prefix.as_bytes());
        let mut ids = Vec::new();

        for item in iter {
            let (key, _) = item.map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            let key_str = std::str::from_utf8(&key).map_err(|e| VectraError::StorageError {
                message: e.to_string(),
            })?;
            if !key_str.starts_with(prefix) {
                break;
            }
            // Key format: "ao:{src_uuid}:{edge_uuid}" or "ai:{tgt_uuid}:{edge_uuid}"
            // We need the edge_uuid which is the last segment
            if let Some(edge_uuid_str) = key_str.rsplit(':').next() {
                // Actually the format has 3 parts: prefix:node_uuid:edge_uuid
                // rsplitn(2, ':') gives [edge_uuid, "ao:node_uuid"]
                let uuid =
                    Uuid::parse_str(edge_uuid_str).map_err(|e| VectraError::StorageError {
                        message: e.to_string(),
                    })?;
                ids.push(uuid);
            }
        }

        Ok(ids)
    }
}

// ─── Key formatting ──────────────────────────────────────────────

fn node_key(id: Uuid) -> Vec<u8> {
    format!("n:{}", id).into_bytes()
}

fn edge_key(id: Uuid) -> Vec<u8> {
    format!("e:{}", id).into_bytes()
}

fn adj_out_key(source: Uuid, edge: Uuid) -> Vec<u8> {
    format!("ao:{}:{}", source, edge).into_bytes()
}

fn adj_in_key(target: Uuid, edge: Uuid) -> Vec<u8> {
    format!("ai:{}:{}", target, edge).into_bytes()
}

fn label_index_key(label: &str, node: Uuid) -> Vec<u8> {
    format!("li:{}:{}", label, node).into_bytes()
}

fn reltype_index_key(rel_type: &str, edge: Uuid) -> Vec<u8> {
    format!("ri:{}:{}", rel_type, edge).into_bytes()
}

/// Hash a GraphValue to a stable string for use in index keys.
fn value_hash(value: &GraphValue) -> String {
    match value {
        GraphValue::Null => "null".to_string(),
        GraphValue::Bool(b) => format!("b:{}", b),
        GraphValue::Integer(n) => format!("i:{}", n),
        GraphValue::Float(f) => format!("f:{}", f),
        GraphValue::String(s) => format!("s:{}", s),
        _ => format!("h:{:x}", {
            // Simple hash for complex types
            let bytes = format!("{:?}", value);
            bytes
                .bytes()
                .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))
        }),
    }
}

fn prop_index_key(label: &str, property: &str, value: &GraphValue, node: Uuid) -> Vec<u8> {
    format!("pi:{}:{}:{}:{}", label, property, value_hash(value), node).into_bytes()
}

fn prop_index_prefix(label: &str, property: &str, value: &GraphValue) -> String {
    format!("pi:{}:{}:{}:", label, property, value_hash(value))
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

    fn test_storage() -> (GraphStorage, TempDir) {
        let dir = TempDir::new().unwrap();
        let storage = GraphStorage::open(dir.path()).unwrap();
        (storage, dir)
    }

    #[test]
    fn test_create_and_get_node() {
        let (storage, _dir) = test_storage();
        let mut props = HashMap::new();
        props.insert("name".to_string(), GraphValue::String("Alice".to_string()));
        props.insert("age".to_string(), GraphValue::Integer(30));

        let id = storage.create_node(&["Person".to_string()], props).unwrap();
        let node = storage.get_node(id).unwrap().unwrap();

        assert_eq!(node.labels, vec!["Person"]);
        assert_eq!(
            node.properties.get("name"),
            Some(&GraphValue::String("Alice".to_string()))
        );
        assert_eq!(node.properties.get("age"), Some(&GraphValue::Integer(30)));
    }

    #[test]
    fn test_create_and_get_edge() {
        let (storage, _dir) = test_storage();
        let alice = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let bob = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();

        let mut props = HashMap::new();
        props.insert("since".to_string(), GraphValue::Integer(2020));

        let edge_id = storage.create_edge(alice, bob, "KNOWS", props).unwrap();
        let edge = storage.get_edge(edge_id).unwrap().unwrap();

        assert_eq!(edge.source, alice);
        assert_eq!(edge.target, bob);
        assert_eq!(edge.rel_type, "KNOWS");
        assert_eq!(
            edge.properties.get("since"),
            Some(&GraphValue::Integer(2020))
        );
    }

    #[test]
    fn test_nodes_by_label() {
        let (storage, _dir) = test_storage();
        let _a = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let _b = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let _c = storage
            .create_node(&["Document".to_string()], HashMap::new())
            .unwrap();

        let people = storage.nodes_by_label("Person").unwrap();
        assert_eq!(people.len(), 2);

        let docs = storage.nodes_by_label("Document").unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn test_expand_out() {
        let (storage, _dir) = test_storage();
        let alice = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let bob = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let carol = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();

        storage
            .create_edge(alice, bob, "KNOWS", HashMap::new())
            .unwrap();
        storage
            .create_edge(alice, carol, "WORKS_WITH", HashMap::new())
            .unwrap();

        // Expand all outgoing
        let all = storage.expand_out(alice, &[]).unwrap();
        assert_eq!(all.len(), 2);

        // Expand filtered by type
        let knows = storage.expand_out(alice, &["KNOWS".to_string()]).unwrap();
        assert_eq!(knows.len(), 1);
        assert_eq!(knows[0].1.id, bob);
    }

    #[test]
    fn test_expand_in() {
        let (storage, _dir) = test_storage();
        let alice = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let bob = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();

        storage
            .create_edge(alice, bob, "KNOWS", HashMap::new())
            .unwrap();

        let incoming = storage.expand_in(bob, &[]).unwrap();
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].1.id, alice);
    }

    #[test]
    fn test_delete_node_detach() {
        let (storage, _dir) = test_storage();
        let alice = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let bob = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        storage
            .create_edge(alice, bob, "KNOWS", HashMap::new())
            .unwrap();

        // Can't delete without detach
        assert!(storage.delete_node(alice, false).is_err());

        // Can delete with detach
        storage.delete_node(alice, true).unwrap();
        assert!(storage.get_node(alice).unwrap().is_none());

        // Edge should also be gone
        let incoming = storage.expand_in(bob, &[]).unwrap();
        assert!(incoming.is_empty());
    }

    #[test]
    fn test_set_node_property() {
        let (storage, _dir) = test_storage();
        let id = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();

        storage
            .set_node_property(id, "name", GraphValue::String("Alice".to_string()))
            .unwrap();

        let node = storage.get_node(id).unwrap().unwrap();
        assert_eq!(
            node.properties.get("name"),
            Some(&GraphValue::String("Alice".to_string()))
        );
    }

    #[test]
    fn test_edge_requires_existing_nodes() {
        let (storage, _dir) = test_storage();
        let alice = storage
            .create_node(&["Person".to_string()], HashMap::new())
            .unwrap();
        let fake = Uuid::new_v4();

        assert!(storage
            .create_edge(alice, fake, "KNOWS", HashMap::new())
            .is_err());
    }
}
