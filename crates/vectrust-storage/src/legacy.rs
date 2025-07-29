use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use vectrust_core::*;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

/// Legacy storage format compatible with existing vectra-enhanced indexes
pub struct LegacyStorage {
    path: PathBuf,
    index_name: String,
    cache: tokio::sync::RwLock<Option<LegacyIndexFile>>,
}

/// Exact format matching Node.js index.json structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyIndexFile {
    pub version: u32,
    pub metadata_config: MetadataConfig,
    pub items: Vec<VectorItem>,
}

impl LegacyStorage {
    pub fn new(path: &Path, index_name: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            index_name: index_name.to_string(),
            cache: tokio::sync::RwLock::new(None),
        })
    }
    
    fn index_path(&self) -> PathBuf {
        self.path.join(&self.index_name)
    }
    
    async fn load_index(&self) -> Result<LegacyIndexFile> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref index) = *cache {
                return Ok(index.clone());
            }
        }
        
        // Load from disk
        let path = self.index_path();
        if !path.exists() {
            return Err(VectraError::IndexNotFound { 
                path: path.to_string_lossy().to_string() 
            });
        }
        
        let content = fs::read_to_string(&path).await?;
        let index: LegacyIndexFile = serde_json::from_str(&content)?;
        
        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(index.clone());
        }
        
        Ok(index)
    }
    
    async fn save_index(&self, index: &LegacyIndexFile) -> Result<()> {
        let path = self.index_path();
        
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Write atomically via temp file
        let temp_path = path.with_extension("tmp");
        let content = serde_json::to_string_pretty(index)?;
        fs::write(&temp_path, content).await?;
        fs::rename(&temp_path, &path).await?;
        
        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(index.clone());
        }
        
        Ok(())
    }
    
    async fn load_metadata(&self, id: &Uuid) -> Result<Option<serde_json::Value>> {
        let metadata_path = self.path.join(format!("{}.json", id));
        
        if !metadata_path.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(metadata_path).await?;
        let metadata: serde_json::Value = serde_json::from_str(&content)?;
        Ok(Some(metadata))
    }
    
    async fn save_metadata(&self, id: &Uuid, metadata: &serde_json::Value) -> Result<()> {
        let metadata_path = self.path.join(format!("{}.json", id));
        let content = serde_json::to_string_pretty(metadata)?;
        fs::write(metadata_path, content).await?;
        Ok(())
    }
    
    async fn delete_metadata(&self, id: &Uuid) -> Result<()> {
        let metadata_path = self.path.join(format!("{}.json", id));
        if metadata_path.exists() {
            fs::remove_file(metadata_path).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for LegacyStorage {
    async fn exists(&self) -> bool {
        self.index_path().exists()
    }
    
    async fn create_index(&mut self, config: &CreateIndexConfig) -> Result<()> {
        let index_path = self.index_path();
        
        if index_path.exists() && !config.delete_if_exists {
            return Err(VectraError::IndexAlreadyExists { 
                path: index_path.to_string_lossy().to_string() 
            });
        }
        
        let index = LegacyIndexFile {
            version: config.version,
            metadata_config: config.metadata_config.clone(),
            items: Vec::new(),
        };
        
        self.save_index(&index).await?;
        Ok(())
    }
    
    async fn get_item(&self, id: &Uuid) -> Result<Option<VectorItem>> {
        let index = self.load_index().await?;
        
        // Find item in index
        let item = index.items.iter().find(|item| &item.id == id);
        
        if let Some(mut item) = item.cloned() {
            // Load external metadata if present
            if let Some(external_metadata) = self.load_metadata(id).await? {
                item.metadata = external_metadata;
            }
            Ok(Some(item))
        } else {
            Ok(None)
        }
    }
    
    async fn insert_item(&mut self, item: &VectorItem) -> Result<()> {
        let mut index = self.load_index().await?;
        
        // Check if item already exists
        if index.items.iter().any(|existing| existing.id == item.id) {
            return Err(VectraError::Storage { 
                message: format!("Item with ID {} already exists", item.id) 
            });
        }
        
        // Handle large metadata (save externally if > 1KB)
        let mut item_to_store = item.clone();
        let metadata_size = serde_json::to_string(&item.metadata)?.len();
        
        if metadata_size > 1024 {
            // Save metadata externally
            self.save_metadata(&item.id, &item.metadata).await?;
            item_to_store.metadata = serde_json::Value::Object(serde_json::Map::new());
        }
        
        // Add to index
        index.items.push(item_to_store);
        self.save_index(&index).await?;
        
        Ok(())
    }
    
    async fn update_item(&mut self, item: &VectorItem) -> Result<()> {
        let mut index = self.load_index().await?;
        
        // Find and update item
        let position = index.items.iter().position(|existing| existing.id == item.id)
            .ok_or(VectraError::ItemNotFound)?;
        
        // Handle metadata storage
        let mut item_to_store = item.clone();
        let metadata_size = serde_json::to_string(&item.metadata)?.len();
        
        if metadata_size > 1024 {
            self.save_metadata(&item.id, &item.metadata).await?;
            item_to_store.metadata = serde_json::Value::Object(serde_json::Map::new());
        } else {
            // Remove external metadata file if it exists
            self.delete_metadata(&item.id).await?;
        }
        
        index.items[position] = item_to_store;
        self.save_index(&index).await?;
        
        Ok(())
    }
    
    async fn delete_item(&mut self, id: &Uuid) -> Result<()> {
        let mut index = self.load_index().await?;
        
        // Remove from index
        let original_len = index.items.len();
        index.items.retain(|item| &item.id != id);
        
        if index.items.len() == original_len {
            return Err(VectraError::ItemNotFound);
        }
        
        // Delete external metadata if exists
        self.delete_metadata(id).await?;
        
        self.save_index(&index).await?;
        Ok(())
    }
    
    async fn list_items(&self, options: Option<ListOptions>) -> Result<Vec<VectorItem>> {
        let index = self.load_index().await?;
        let mut items = index.items.clone();
        
        // Load external metadata for all items
        for item in &mut items {
            if let Some(external_metadata) = self.load_metadata(&item.id).await? {
                item.metadata = external_metadata;
            }
        }
        
        // Apply pagination if specified
        if let Some(opts) = options {
            let offset = opts.offset.unwrap_or(0);
            let limit = opts.limit.unwrap_or(items.len());
            
            if offset < items.len() {
                let end = std::cmp::min(offset + limit, items.len());
                items = items[offset..end].to_vec();
            } else {
                items.clear();
            }
        }
        
        Ok(items)
    }
    
    async fn query_items(&self, query: &Query) -> Result<Vec<QueryResult>> {
        let index = self.load_index().await?;
        
        if let Some(ref query_vector) = query.vector {
            // Vector similarity search using proper vector operations
            let mut results = Vec::new();
            
            for item in &index.items {
                if item.deleted {
                    continue;
                }
                
                // Validate vector compatibility
                if !VectorOps::compatible_dimensions(query_vector, &item.vector) {
                    continue;
                }
                
                // Calculate similarity using cosine similarity (legacy format default)
                let similarity = VectorOps::calculate_similarity(
                    query_vector, 
                    &item.vector, 
                    &DistanceMetric::Cosine
                );
                
                // Only include valid similarities
                if similarity.is_finite() {
                    results.push(QueryResult {
                        item: item.clone(),
                        score: similarity,
                    });
                }
            }
            
            // Sort by score descending
            results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            
            // Apply limit
            results.truncate(query.top_k);
            
            // Load external metadata for results
            for result in &mut results {
                if let Some(external_metadata) = self.load_metadata(&result.item.id).await? {
                    result.item.metadata = external_metadata;
                }
            }
            
            Ok(results)
        } else if let Some(ref _text_query) = query.text {
            // Text-only search not implemented in legacy format
            // In a full implementation, this would use BM25 or similar
            Ok(Vec::new())
        } else {
            // No query criteria provided
            Ok(Vec::new())
        }
    }
    
    async fn begin_transaction(&mut self) -> Result<()> {
        // Legacy format doesn't support transactions
        Ok(())
    }
    
    async fn commit_transaction(&mut self) -> Result<()> {
        // Legacy format doesn't support transactions
        Ok(())
    }
    
    async fn rollback_transaction(&mut self) -> Result<()> {
        // Legacy format doesn't support transactions
        Ok(())
    }
    
    async fn delete_index(&mut self) -> Result<()> {
        let index_path = self.index_path();
        
        if index_path.exists() {
            // Remove index file
            fs::remove_file(&index_path).await?;
            
            // Remove all metadata files
            let mut dir = fs::read_dir(&self.path).await?;
            let mut metadata_files = Vec::new();
            
            while let Some(entry) = dir.next_entry().await? {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "json" && path != index_path {
                        // Check if it's a UUID filename (metadata file)
                        if let Some(stem) = path.file_stem() {
                            if let Some(stem_str) = stem.to_str() {
                                if Uuid::parse_str(stem_str).is_ok() {
                                    metadata_files.push(path);
                                }
                            }
                        }
                    }
                }
            }
            
            // Delete metadata files
            for metadata_file in metadata_files {
                fs::remove_file(metadata_file).await?;
            }
        }
        
        // Clear cache
        {
            let mut cache = self.cache.write().await;
            *cache = None;
        }
        
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<IndexStats> {
        if !self.exists().await {
            return Ok(IndexStats {
                items: 0,
                size: 0,
                dimensions: None,
                distance_metric: DistanceMetric::Cosine,
            });
        }
        
        let index = self.load_index().await?;
        let dimensions = index.items.first().map(|item| item.vector.len());
        
        // Calculate total size
        let index_size = fs::metadata(self.index_path()).await?.len();
        
        Ok(IndexStats {
            items: index.items.len(),
            size: index_size,
            dimensions,
            distance_metric: DistanceMetric::Cosine, // Legacy format always uses cosine
        })
    }
}

