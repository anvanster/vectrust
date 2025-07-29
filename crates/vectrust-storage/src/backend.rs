use async_trait::async_trait;
use std::path::Path;
use vectrust_core::*;

/// Trait for storage backend implementations
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn exists(&self) -> bool;
    async fn create_index(&mut self, config: &CreateIndexConfig) -> Result<()>;
    async fn get_item(&self, id: &uuid::Uuid) -> Result<Option<VectorItem>>;
    async fn insert_item(&mut self, item: &VectorItem) -> Result<()>;
    async fn insert_items(&mut self, items: &[VectorItem]) -> Result<()> {
        // Default implementation - can be overridden for better performance
        for item in items {
            self.insert_item(item).await?;
        }
        Ok(())
    }
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

pub struct Storage;

impl Storage {
    /// Auto-detect storage format and return appropriate backend
    pub fn auto_detect(path: &Path, index_name: &str) -> Result<Box<dyn StorageBackend>> {
        let index_path = path.join(index_name);
        let manifest_path = path.join("manifest.json");
        
        if manifest_path.exists() {
            // V2 optimized format
            Ok(Box::new(crate::OptimizedStorage::new(path)?))
        } else if index_path.exists() {
            // V1 legacy format
            Ok(Box::new(crate::LegacyStorage::new(path, index_name)?))
        } else {
            // New index - use legacy format for now since optimized isn't fully implemented
            Ok(Box::new(crate::LegacyStorage::new(path, index_name)?))
        }
    }
}