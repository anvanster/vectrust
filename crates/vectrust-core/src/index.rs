use crate::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::Utc;
use serde::{Serialize, Deserialize};

// We'll define a trait here to avoid cyclic dependencies
use async_trait::async_trait;

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn exists(&self) -> bool;
    async fn create_index(&mut self, config: &CreateIndexConfig) -> Result<()>;
    async fn get_item(&self, id: &uuid::Uuid) -> Result<Option<VectorItem>>;
    async fn insert_item(&mut self, item: &VectorItem) -> Result<()>;
    async fn update_item(&mut self, item: &VectorItem) -> Result<()>;
    async fn delete_item(&mut self, id: &uuid::Uuid) -> Result<()>;
    async fn list_items(&self, options: Option<ListOptions>) -> Result<Vec<VectorItem>>;
    async fn query_items(&self, query: &Query) -> Result<Vec<QueryResult>>;
    async fn begin_transaction(&mut self) -> Result<()>;
    async fn commit_transaction(&mut self) -> Result<()>;
    async fn rollback_transaction(&mut self) -> Result<()>;
    async fn delete_index(&mut self) -> Result<()>;
    async fn get_stats(&self) -> Result<IndexStats>;
}

/// Configuration matching Node.js CreateIndexConfig
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIndexConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    
    #[serde(default)]
    pub delete_if_exists: bool,
    
    #[serde(default = "default_distance_metric")]
    pub distance_metric: DistanceMetric,
    
    #[serde(default)]
    pub metadata_config: MetadataConfig,
    
    #[serde(default)]
    pub hnsw_config: HnswConfig,
}

fn default_version() -> u32 { 1 }
fn default_distance_metric() -> DistanceMetric { DistanceMetric::Cosine }

impl Default for CreateIndexConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            delete_if_exists: false,
            distance_metric: default_distance_metric(),
            metadata_config: MetadataConfig::default(),
            hnsw_config: HnswConfig::default(),
        }
    }
}

/// Main LocalIndex implementation with full Node.js API compatibility
pub struct LocalIndex {
    #[allow(dead_code)]
    path: PathBuf,
    #[allow(dead_code)]
    index_name: String,
    storage: Arc<RwLock<Box<dyn StorageBackend>>>,
    config: Arc<RwLock<Option<CreateIndexConfig>>>,
}

impl LocalIndex {
    /// Creates a new LocalIndex instance
    /// Maintains exact Node.js constructor compatibility
    pub fn new<P: AsRef<Path>>(folder_path: P, index_name: Option<String>) -> Result<Self> {
        let path = folder_path.as_ref().to_path_buf();
        let index_name = index_name.unwrap_or_else(|| "index.json".to_string());
        
        // Auto-detect storage backend based on existing format
        // This will be implemented when we can access vectra_storage from vectra_core
        let storage = create_dummy_storage();
        
        Ok(Self {
            path,
            index_name,
            storage: Arc::new(RwLock::new(storage)),
            config: Arc::new(RwLock::new(None)),
        })
    }
    
    /// Creates an index with optional configuration
    pub async fn create_index(&self, config: Option<CreateIndexConfig>) -> Result<()> {
        let config = config.unwrap_or_default();
        
        // Handle delete_if_exists
        if config.delete_if_exists && self.is_index_created().await {
            self.delete_index().await?;
        }
        
        // Create index through storage backend
        let mut storage = self.storage.write().await;
        storage.create_index(&config).await?;
        
        // Store config
        *self.config.write().await = Some(config);
        
        Ok(())
    }
    
    /// Checks if index exists
    pub async fn is_index_created(&self) -> bool {
        let storage = self.storage.read().await;
        storage.exists().await
    }
    
    /// Inserts a new item
    pub async fn insert_item(&self, item: impl Into<VectorItem>) -> Result<VectorItem> {
        let mut item = item.into();
        
        // Ensure ID
        if item.id == Uuid::default() {
            item.id = Uuid::new_v4();
        }
        
        // Update timestamps
        let now = Utc::now();
        item.created_at = now;
        item.updated_at = now;
        
        // Insert through storage
        let mut storage = self.storage.write().await;
        storage.insert_item(&item).await?;
        
        Ok(item)
    }
    
    /// Updates an existing item (partial update)
    pub async fn update_item(&self, update: UpdateRequest) -> Result<UpdateResult> {
        let mut storage = self.storage.write().await;
        
        // Get existing item
        let mut item = storage.get_item(&update.id).await?
            .ok_or(VectraError::ItemNotFound)?;
        
        // Apply updates
        if let Some(vector) = update.vector {
            item.vector = vector;
        }
        
        if let Some(metadata) = update.metadata {
            merge_json(&mut item.metadata, metadata);
        }
        
        // Update version and timestamp
        item.version += 1;
        item.updated_at = Utc::now();
        
        // Save
        storage.update_item(&item).await?;
        
        Ok(UpdateResult {
            id: item.id,
            version: item.version,
        })
    }
    
    /// Upserts an item (insert or update)
    pub async fn upsert_item(&self, item: impl Into<VectorItem>) -> Result<VectorItem> {
        let item = item.into();
        let mut storage = self.storage.write().await;
        
        if storage.get_item(&item.id).await?.is_some() {
            // Update existing
            storage.update_item(&item).await?;
        } else {
            // Insert new
            storage.insert_item(&item).await?;
        }
        
        Ok(item)
    }
    
    /// Gets an item by ID
    pub async fn get_item(&self, id: &Uuid) -> Result<Option<VectorItem>> {
        let storage = self.storage.read().await;
        storage.get_item(id).await
    }
    
    /// Deletes an item
    pub async fn delete_item(&self, id: &Uuid) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.delete_item(id).await
    }
    
    /// Lists all items with optional pagination
    pub async fn list_items(&self, options: Option<ListOptions>) -> Result<Vec<VectorItem>> {
        let storage = self.storage.read().await;
        storage.list_items(options).await
    }
    
    /// Query items - maintains Node.js signature compatibility
    pub async fn query_items(
        &self,
        vector: Vec<f32>,
        top_k: Option<u32>,
        filter: Option<serde_json::Value>,
    ) -> Result<Vec<QueryResult>> {
        self.query_items_extended(vector, None, top_k, filter).await
    }
    
    /// Extended query with text search
    pub async fn query_items_extended(
        &self,
        vector: Vec<f32>,
        text_query: Option<String>,
        top_k: Option<u32>,
        filter: Option<serde_json::Value>,
    ) -> Result<Vec<QueryResult>> {
        let storage = self.storage.read().await;
        
        let query = Query {
            vector: Some(vector),
            text: text_query,
            top_k: top_k.unwrap_or(10) as usize,
            filter,
        };
        
        storage.query_items(&query).await
    }
    
    /// Begins an update transaction
    pub async fn begin_update(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.begin_transaction().await
    }
    
    /// Ends an update transaction
    pub async fn end_update(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.commit_transaction().await
    }
    
    /// Cancels an update transaction (now async for safety)
    pub async fn cancel_update(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.rollback_transaction().await
    }
    
    /// Deletes the entire index
    pub async fn delete_index(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.delete_index().await
    }
    
    /// Gets index statistics
    pub async fn get_stats(&self) -> Result<IndexStats> {
        let storage = self.storage.read().await;
        storage.get_stats().await
    }
}

/// Helper function to merge JSON objects (for metadata updates)
fn merge_json(target: &mut serde_json::Value, source: serde_json::Value) {
    if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in source_obj {
            target_obj.insert(key.clone(), value.clone());
        }
    } else {
        *target = source;
    }
}

// Dummy storage implementation for compilation
struct DummyStorage;

#[async_trait]
impl StorageBackend for DummyStorage {
    async fn exists(&self) -> bool { false }
    async fn create_index(&mut self, _config: &CreateIndexConfig) -> Result<()> { Ok(()) }
    async fn get_item(&self, _id: &uuid::Uuid) -> Result<Option<VectorItem>> { Ok(None) }
    async fn insert_item(&mut self, _item: &VectorItem) -> Result<()> { Ok(()) }
    async fn update_item(&mut self, _item: &VectorItem) -> Result<()> { Ok(()) }
    async fn delete_item(&mut self, _id: &uuid::Uuid) -> Result<()> { Ok(()) }
    async fn list_items(&self, _options: Option<ListOptions>) -> Result<Vec<VectorItem>> { Ok(Vec::new()) }
    async fn query_items(&self, _query: &Query) -> Result<Vec<QueryResult>> { Ok(Vec::new()) }
    async fn begin_transaction(&mut self) -> Result<()> { Ok(()) }
    async fn commit_transaction(&mut self) -> Result<()> { Ok(()) }
    async fn rollback_transaction(&mut self) -> Result<()> { Ok(()) }
    async fn delete_index(&mut self) -> Result<()> { Ok(()) }
    async fn get_stats(&self) -> Result<IndexStats> { 
        Ok(IndexStats {
            items: 0,
            size: 0,
            dimensions: None,
            distance_metric: DistanceMetric::Cosine,
        })
    }
}

fn create_dummy_storage() -> Box<dyn StorageBackend> {
    Box::new(DummyStorage)
}