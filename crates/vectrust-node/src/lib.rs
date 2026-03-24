use napi::{Error, Result};
use napi_derive::napi;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;
use uuid::Uuid;
use vectrust::{
    CreateIndexConfig, GraphIndex as RustGraphIndex, GraphValue, ListOptions,
    LocalIndex as RustLocalIndex, VectorItem,
};

/// Node.js binding for LocalIndex
#[napi]
pub struct LocalIndex {
    inner: Arc<Mutex<RustLocalIndex>>,
}

#[napi]
impl LocalIndex {
    #[napi(constructor)]
    pub fn new(folder_path: String, index_name: Option<String>) -> Result<Self> {
        let inner = RustLocalIndex::new(folder_path, index_name)
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    #[napi]
    pub async fn create_index(&self, config: Option<String>) -> Result<()> {
        let config = if let Some(config_str) = config {
            let config: CreateIndexConfig =
                serde_json::from_str(&config_str).map_err(|e| Error::from_reason(e.to_string()))?;
            Some(config)
        } else {
            None
        };

        let index = self.inner.lock().await;
        index
            .create_index(config)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn is_index_created(&self) -> Result<bool> {
        let index = self.inner.lock().await;
        Ok(index.is_index_created().await)
    }

    #[napi]
    pub async fn insert_item(&self, item_json: String) -> Result<String> {
        let vector_item: VectorItem =
            serde_json::from_str(&item_json).map_err(|e| Error::from_reason(e.to_string()))?;

        let index = self.inner.lock().await;
        let result = index
            .insert_item(vector_item)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        serde_json::to_string(&result).map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn get_item(&self, id: String) -> Result<Option<String>> {
        let uuid = Uuid::parse_str(&id).map_err(|e| Error::from_reason(e.to_string()))?;

        let index = self.inner.lock().await;
        let result = index
            .get_item(&uuid)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        match result {
            Some(item) => {
                let json =
                    serde_json::to_string(&item).map_err(|e| Error::from_reason(e.to_string()))?;
                Ok(Some(json))
            }
            None => Ok(None),
        }
    }

    #[napi]
    pub async fn query_items(
        &self,
        vector: Vec<f64>,
        top_k: Option<u32>,
        filter: Option<String>,
    ) -> Result<String> {
        // Convert f64 to f32 for compatibility
        let vector: Vec<f32> = vector.into_iter().map(|v| v as f32).collect();

        let filter = if let Some(filter_str) = filter {
            let filter_value: serde_json::Value =
                serde_json::from_str(&filter_str).map_err(|e| Error::from_reason(e.to_string()))?;
            Some(filter_value)
        } else {
            None
        };

        let index = self.inner.lock().await;
        let results = index
            .query_items(vector, top_k, filter)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        serde_json::to_string(&results).map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn delete_item(&self, id: String) -> Result<()> {
        let uuid = Uuid::parse_str(&id).map_err(|e| Error::from_reason(e.to_string()))?;

        let index = self.inner.lock().await;
        index
            .delete_item(&uuid)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn list_items(&self, options: Option<String>) -> Result<String> {
        let list_options = if let Some(opts_str) = options {
            let opts: ListOptions =
                serde_json::from_str(&opts_str).map_err(|e| Error::from_reason(e.to_string()))?;
            Some(opts)
        } else {
            None
        };

        let index = self.inner.lock().await;
        let items = index
            .list_items(list_options)
            .await
            .map_err(|e| Error::from_reason(e.to_string()))?;

        serde_json::to_string(&items).map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn begin_update(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index
            .begin_update()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn end_update(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index
            .end_update()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn cancel_update(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index
            .cancel_update()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn delete_index(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index
            .delete_index()
            .await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}

/// Node.js binding for GraphIndex — graph + vector database with Cypher queries.
#[napi]
pub struct GraphIndex {
    inner: Arc<StdMutex<RustGraphIndex>>,
}

#[napi]
impl GraphIndex {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let inner = RustGraphIndex::open(&path).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(StdMutex::new(inner)),
        })
    }

    /// Execute a Cypher query and return results as JSON.
    #[napi]
    pub fn cypher(&self, query: String) -> Result<String> {
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let result = db
            .cypher(&query)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        query_result_to_json(&result)
    }

    /// Execute a Cypher query with parameter bindings (JSON object).
    #[napi]
    pub fn cypher_with_params(&self, query: String, params: String) -> Result<String> {
        let params: serde_json::Value =
            serde_json::from_str(&params).map_err(|e| Error::from_reason(e.to_string()))?;
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let result = db
            .cypher_with_params(&query, params)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        query_result_to_json(&result)
    }

    /// Create a node with labels and properties (JSON). Returns node JSON.
    #[napi]
    pub fn create_node(&self, labels: Vec<String>, properties: String) -> Result<String> {
        let props: serde_json::Value =
            serde_json::from_str(&properties).map_err(|e| Error::from_reason(e.to_string()))?;
        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let node = db
            .create_node(&label_refs, props)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let json_val: serde_json::Value = GraphValue::Node(node).into();
        serde_json::to_string(&json_val).map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Create a node with labels, properties, and an embedding vector. Returns node JSON.
    #[napi]
    pub fn create_node_with_vector(
        &self,
        labels: Vec<String>,
        properties: String,
        vector: Vec<f64>,
    ) -> Result<String> {
        let props: serde_json::Value =
            serde_json::from_str(&properties).map_err(|e| Error::from_reason(e.to_string()))?;
        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let vec_f32: Vec<f32> = vector.into_iter().map(|v| v as f32).collect();
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let node = db
            .create_node_with_vector(&label_refs, props, vec_f32)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let json_val: serde_json::Value = GraphValue::Node(node).into();
        serde_json::to_string(&json_val).map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Create an edge between two nodes. Returns edge JSON.
    #[napi]
    pub fn create_edge(
        &self,
        source_id: String,
        target_id: String,
        rel_type: String,
        properties: String,
    ) -> Result<String> {
        let source = Uuid::parse_str(&source_id).map_err(|e| Error::from_reason(e.to_string()))?;
        let target = Uuid::parse_str(&target_id).map_err(|e| Error::from_reason(e.to_string()))?;
        let props: serde_json::Value =
            serde_json::from_str(&properties).map_err(|e| Error::from_reason(e.to_string()))?;
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let edge = db
            .create_edge(source, target, &rel_type, props)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let json_val: serde_json::Value = GraphValue::Edge(edge).into();
        serde_json::to_string(&json_val).map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Get a node by ID. Returns node JSON or null.
    #[napi]
    pub fn get_node(&self, id: String) -> Result<Option<String>> {
        let uuid = Uuid::parse_str(&id).map_err(|e| Error::from_reason(e.to_string()))?;
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let node = db
            .get_node(uuid)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        match node {
            Some(n) => {
                let json_val: serde_json::Value = GraphValue::Node(n).into();
                let json = serde_json::to_string(&json_val)
                    .map_err(|e| Error::from_reason(e.to_string()))?;
                Ok(Some(json))
            }
            None => Ok(None),
        }
    }

    /// Delete a node. If detach is true, also deletes all connected edges.
    #[napi]
    pub fn delete_node(&self, id: String, detach: bool) -> Result<()> {
        let uuid = Uuid::parse_str(&id).map_err(|e| Error::from_reason(e.to_string()))?;
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        db.delete_node(uuid, detach)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Get all nodes with a given label. Returns JSON array.
    #[napi]
    pub fn nodes_by_label(&self, label: String) -> Result<String> {
        let db = self
            .inner
            .lock()
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let nodes = db
            .nodes_by_label(&label)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        let json_nodes: Vec<serde_json::Value> = nodes
            .into_iter()
            .map(|n| GraphValue::Node(n).into())
            .collect();
        serde_json::to_string(&json_nodes).map_err(|e| Error::from_reason(e.to_string()))
    }
}

fn query_result_to_json(result: &vectrust::GraphQueryResult) -> Result<String> {
    let rows_json: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let obj: serde_json::Map<String, serde_json::Value> = row
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::from(v.clone())))
                .collect();
            serde_json::Value::Object(obj)
        })
        .collect();

    let output = serde_json::json!({
        "columns": result.columns,
        "rows": rows_json,
    });

    serde_json::to_string(&output).map_err(|e| Error::from_reason(e.to_string()))
}
