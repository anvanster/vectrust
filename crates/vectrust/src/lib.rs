pub use vectrust_core::*;

use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// High-level LocalIndex that integrates all components
pub struct LocalIndex {
    storage: Arc<RwLock<Box<dyn vectrust_storage::StorageBackend>>>,
    #[allow(dead_code)]
    path: std::path::PathBuf,
    #[allow(dead_code)]
    index_name: String,
}

impl LocalIndex {
    /// Create a new LocalIndex with auto-detected storage backend
    pub fn new<P: AsRef<Path>>(folder_path: P, index_name: Option<String>) -> Result<Self> {
        let path = folder_path.as_ref().to_path_buf();
        let index_name = index_name.unwrap_or_else(|| "index.json".to_string());
        
        // Auto-detect storage format and create appropriate backend
        let storage = vectrust_storage::Storage::auto_detect(&path, &index_name)?;
        
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            path,
            index_name,
        })
    }
    
    /// Create an index with configuration
    pub async fn create_index(&self, config: Option<CreateIndexConfig>) -> Result<()> {
        let config = config.unwrap_or_default();
        let mut storage = self.storage.write().await;
        storage.create_index(&config).await
    }
    
    /// Check if index exists
    pub async fn is_index_created(&self) -> bool {
        let storage = self.storage.read().await;
        storage.exists().await
    }
    
    /// Insert a new item
    pub async fn insert_item(&self, mut item: VectorItem) -> Result<VectorItem> {
        // Ensure ID is set
        if item.id == uuid::Uuid::default() {
            item.id = uuid::Uuid::new_v4();
        }
        
        // Validate vector
        if !VectorOps::is_valid_vector(&item.vector) {
            return Err(VectraError::VectorValidation {
                message: "Vector contains NaN or infinite values".to_string(),
            });
        }
        
        // Update timestamps
        let now = chrono::Utc::now();
        item.created_at = now;
        item.updated_at = now;
        
        let mut storage = self.storage.write().await;
        storage.insert_item(&item).await?;
        
        Ok(item)
    }
    
    /// Insert multiple items efficiently using bulk operations
    pub async fn insert_items(&self, mut items: Vec<VectorItem>) -> Result<Vec<VectorItem>> {
        if items.is_empty() {
            return Ok(items);
        }
        
        let now = chrono::Utc::now();
        
        // Process all items
        for item in &mut items {
            // Ensure ID is set
            if item.id == uuid::Uuid::default() || item.id.is_nil() {
                item.id = uuid::Uuid::new_v4();
            }
            
            // Validate vector
            if !VectorOps::is_valid_vector(&item.vector) {
                return Err(VectraError::VectorValidation {
                    message: "Vector contains NaN or infinite values".to_string(),
                });
            }
            
            // Set timestamps
            if item.created_at.timestamp() == 0 {
                item.created_at = now;
            }
            item.updated_at = now;
        }
        
        let mut storage = self.storage.write().await;
        storage.insert_items(&items).await?;
        
        Ok(items)
    }
    
    /// Get an item by ID
    pub async fn get_item(&self, id: &uuid::Uuid) -> Result<Option<VectorItem>> {
        let storage = self.storage.read().await;
        storage.get_item(id).await
    }
    
    /// Update an existing item
    pub async fn update_item(&self, update: UpdateRequest) -> Result<UpdateResult> {
        let mut storage = self.storage.write().await;
        
        // Get existing item
        let mut item = storage.get_item(&update.id).await?
            .ok_or(VectraError::ItemNotFound)?;
        
        // Apply updates
        if let Some(vector) = update.vector {
            if !VectorOps::is_valid_vector(&vector) {
                return Err(VectraError::VectorValidation {
                    message: "Vector contains NaN or infinite values".to_string(),
                });
            }
            item.vector = vector;
        }
        
        if let Some(metadata) = update.metadata {
            merge_json(&mut item.metadata, metadata);
        }
        
        // Update version and timestamp
        item.version += 1;
        item.updated_at = chrono::Utc::now();
        
        // Save
        storage.update_item(&item).await?;
        
        Ok(UpdateResult {
            id: item.id,
            version: item.version,
        })
    }
    
    /// Delete an item
    pub async fn delete_item(&self, id: &uuid::Uuid) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.delete_item(id).await
    }
    
    /// List all items
    pub async fn list_items(&self, options: Option<ListOptions>) -> Result<Vec<VectorItem>> {
        let storage = self.storage.read().await;
        storage.list_items(options).await
    }
    
    /// Query items with vector similarity
    pub async fn query_items(
        &self,
        vector: Vec<f32>,
        top_k: Option<u32>,
        filter: Option<serde_json::Value>,
    ) -> Result<Vec<QueryResult>> {
        // Validate query vector
        if !VectorOps::is_valid_vector(&vector) {
            return Err(VectraError::VectorValidation {
                message: "Query vector contains NaN or infinite values".to_string(),
            });
        }
        
        let storage = self.storage.read().await;
        let query = Query {
            vector: Some(vector),
            text: None,
            top_k: top_k.unwrap_or(10) as usize,
            filter,
        };
        
        storage.query_items(&query).await
    }
    
    /// Extended query with text search
    pub async fn query_items_extended(
        &self,
        vector: Vec<f32>,
        text_query: Option<String>,
        top_k: Option<u32>,
        filter: Option<serde_json::Value>,
    ) -> Result<Vec<QueryResult>> {
        // Validate query vector
        if !VectorOps::is_valid_vector(&vector) {
            return Err(VectraError::VectorValidation {
                message: "Query vector contains NaN or infinite values".to_string(),
            });
        }
        
        let storage = self.storage.read().await;
        let query = Query {
            vector: Some(vector),
            text: text_query,
            top_k: top_k.unwrap_or(10) as usize,
            filter,
        };
        
        storage.query_items(&query).await
    }
    
    /// Get index statistics
    pub async fn get_stats(&self) -> Result<IndexStats> {
        let storage = self.storage.read().await;
        storage.get_stats().await
    }
    
    /// Delete the entire index
    pub async fn delete_index(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.delete_index().await
    }
    
    /// Begin transaction
    pub async fn begin_update(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.begin_transaction().await
    }
    
    /// End transaction
    pub async fn end_update(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.commit_transaction().await
    }
    
    /// Cancel transaction
    pub async fn cancel_update(&self) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.rollback_transaction().await
    }
}

/// Helper function to merge JSON objects
fn merge_json(target: &mut serde_json::Value, source: serde_json::Value) {
    if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in source_obj {
            target_obj.insert(key.clone(), value.clone());
        }
    } else {
        *target = source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use uuid::Uuid;
    
    #[tokio::test]
    async fn test_local_index_creation() {
        let temp_dir = TempDir::new().unwrap();
        let index = LocalIndex::new(temp_dir.path(), None).unwrap();
        
        assert!(!index.is_index_created().await);
        
        index.create_index(None).await.unwrap();
        assert!(index.is_index_created().await);
    }
    
    #[tokio::test]
    async fn test_insert_and_get_item() {
        let temp_dir = TempDir::new().unwrap();
        let index = LocalIndex::new(temp_dir.path(), None).unwrap();
        index.create_index(None).await.unwrap();
        
        let item = VectorItem {
            id: Uuid::new_v4(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({"test": "data"}),
            ..Default::default()
        };
        
        let inserted = index.insert_item(item.clone()).await.unwrap();
        assert_eq!(inserted.id, item.id);
        
        let retrieved = index.get_item(&item.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, item.id);
    }
    
    #[tokio::test]
    async fn test_vector_similarity_query() {
        let temp_dir = TempDir::new().unwrap();
        let index = LocalIndex::new(temp_dir.path(), None).unwrap();
        index.create_index(None).await.unwrap();
        
        // Insert test vectors
        let item1 = VectorItem {
            id: Uuid::new_v4(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({"name": "item1"}),
            ..Default::default()
        };
        
        let item2 = VectorItem {
            id: Uuid::new_v4(),
            vector: vec![0.0, 1.0, 0.0],
            metadata: serde_json::json!({"name": "item2"}),
            ..Default::default()
        };
        
        index.insert_item(item1.clone()).await.unwrap();
        index.insert_item(item2.clone()).await.unwrap();
        
        // Query with vector similar to item1
        let results = index.query_items(vec![1.0, 0.1, 0.0], Some(2), None).await.unwrap();
        
        assert_eq!(results.len(), 2);
        // First result should be more similar to item1
        assert_eq!(results[0].item.id, item1.id);
        assert!(results[0].score > results[1].score);
    }
    
    #[test]
    fn test_invalid_vector_validation() {
        let invalid_vector = vec![1.0, f32::NAN, 0.0];
        assert!(!VectorOps::is_valid_vector(&invalid_vector));
        
        let valid_vector = vec![1.0, 0.5, 0.0];
        assert!(VectorOps::is_valid_vector(&valid_vector));
    }
}