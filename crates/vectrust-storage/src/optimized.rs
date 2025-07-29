use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use vectrust_core::*;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use rocksdb::{DB, Options};
use memmap2::{MmapMut, MmapOptions};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use bincode;

/// Optimized storage format (v2) with better performance
pub struct OptimizedStorage {
    path: PathBuf,
    db: Arc<RwLock<Option<DB>>>,
    vector_file: Arc<RwLock<Option<std::fs::File>>>,
    vector_mmap: Arc<RwLock<Option<MmapMut>>>,
    manifest: Arc<RwLock<Option<Manifest>>>,
    dimensions: Arc<RwLock<Option<usize>>>,
    // Performance optimization: batch manifest updates
    manifest_dirty: Arc<RwLock<bool>>,
    operations_since_save: Arc<RwLock<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub format: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub dimensions: Option<usize>,
    pub distance_metric: DistanceMetric,
    pub total_items: usize,
    pub vector_file_size: u64,
    pub next_vector_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VectorRecord {
    pub id: Uuid,
    pub offset: u64,
    pub dimensions: usize,
    pub deleted: bool,
}

const METADATA_CF: &str = "metadata";
const VECTOR_INDEX_CF: &str = "vector_index";
const VECTOR_HEADER_SIZE: usize = 8; // u64 for dimensions count

const MANIFEST_SAVE_INTERVAL: u32 = 100; // Save manifest every N operations

impl OptimizedStorage {
    pub fn new(path: &Path) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            db: Arc::new(RwLock::new(None)),
            vector_file: Arc::new(RwLock::new(None)),
            vector_mmap: Arc::new(RwLock::new(None)),
            manifest: Arc::new(RwLock::new(None)),
            dimensions: Arc::new(RwLock::new(None)),
            manifest_dirty: Arc::new(RwLock::new(false)),
            operations_since_save: Arc::new(RwLock::new(0)),
        })
    }
    
    async fn initialize_storage(&self) -> Result<()> {
        // Create directory if it doesn't exist
        if !self.path.exists() {
            std::fs::create_dir_all(&self.path)?;
        }
        
        // Open RocksDB with optimized settings for vector workloads
        let db_path = self.path.join("metadata");
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        
        // Performance optimizations for vector operations
        db_opts.set_max_write_buffer_number(4);
        db_opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        db_opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB
        db_opts.set_level_compaction_dynamic_level_bytes(true);
        db_opts.set_max_bytes_for_level_base(256 * 1024 * 1024); // 256MB
        db_opts.set_max_background_jobs(4);
        db_opts.set_bytes_per_sync(1024 * 1024); // 1MB
        
        let cf_names = vec![METADATA_CF, VECTOR_INDEX_CF];
        let db = DB::open_cf(&db_opts, db_path, cf_names)?;
        
        *self.db.write().await = Some(db);
        
        // Load or create manifest
        if let Some(manifest) = self.load_manifest().await? {
            *self.manifest.write().await = Some(manifest.clone());
            *self.dimensions.write().await = manifest.dimensions;
            
            // Open existing vector file
            let vector_path = self.path.join("vectors.dat");
            if vector_path.exists() {
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&vector_path)?;
                
                let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
                
                *self.vector_file.write().await = Some(file);
                *self.vector_mmap.write().await = Some(mmap);
            }
        }
        
        Ok(())
    }
    
    async fn create_vector_file(&self, initial_size: u64) -> Result<()> {
        let vector_path = self.path.join("vectors.dat");
        
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&vector_path)?;
            
        // Pre-allocate space
        file.seek(SeekFrom::Start(initial_size - 1))?;
        file.write_all(&[0])?;
        file.flush()?;
        
        let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
        
        *self.vector_file.write().await = Some(file);
        *self.vector_mmap.write().await = Some(mmap);
        
        Ok(())
    }
    
    async fn write_vector_to_file(&self, vector: &[f32], offset: u64) -> Result<()> {
        let mut mmap_guard = self.vector_mmap.write().await;
        if let Some(ref mut mmap) = *mmap_guard {
            let start = offset as usize;
            let dimensions = vector.len();
            
            // Write dimensions count first (8 bytes)
            let dim_bytes = (dimensions as u64).to_le_bytes();
            mmap[start..start + 8].copy_from_slice(&dim_bytes);
            
            // Write vector data (4 bytes per f32)
            let vector_start = start + VECTOR_HEADER_SIZE;
            for (i, &value) in vector.iter().enumerate() {
                let value_bytes = value.to_le_bytes();
                let pos = vector_start + (i * 4);
                mmap[pos..pos + 4].copy_from_slice(&value_bytes);
            }
            
            // Don't flush on every write - let OS handle it for better performance
            // mmap.flush()?;
        }
        
        Ok(())
    }
    
    async fn read_vector_from_file(&self, offset: u64, expected_dims: usize) -> Result<Vec<f32>> {
        let mmap_guard = self.vector_mmap.read().await;
        if let Some(ref mmap) = *mmap_guard {
            let start = offset as usize;
            
            // Read dimensions count
            let mut dim_bytes = [0u8; 8];
            dim_bytes.copy_from_slice(&mmap[start..start + 8]);
            let dimensions = u64::from_le_bytes(dim_bytes) as usize;
            
            if dimensions != expected_dims {
                return Err(VectraError::VectorValidation {
                    message: format!("Dimension mismatch: expected {}, got {}", expected_dims, dimensions)
                });
            }
            
            // Read vector data
            let mut vector = Vec::with_capacity(dimensions);
            let vector_start = start + VECTOR_HEADER_SIZE;
            
            for i in 0..dimensions {
                let pos = vector_start + (i * 4);
                let mut value_bytes = [0u8; 4];
                value_bytes.copy_from_slice(&mmap[pos..pos + 4]);
                vector.push(f32::from_le_bytes(value_bytes));
            }
            
            Ok(vector)
        } else {
            Err(VectraError::StorageError { 
                message: "Vector file not initialized".to_string() 
            })
        }
    }
    
    fn manifest_path(&self) -> PathBuf {
        self.path.join("manifest.json")
    }
    
    async fn load_manifest(&self) -> Result<Option<Manifest>> {
        let manifest_path = self.manifest_path();
        
        if !manifest_path.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(manifest_path).await?;
        let manifest: Manifest = serde_json::from_str(&content)?;
        Ok(Some(manifest))
    }
    
    async fn save_manifest_to_disk(&self, manifest: &Manifest) -> Result<()> {
        let manifest_path = self.manifest_path();
        
        // Ensure directory exists
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        let content = serde_json::to_string_pretty(manifest)?;
        fs::write(manifest_path, content).await?;
        
        Ok(())
    }
    
    async fn save_manifest(&self, manifest: &Manifest) -> Result<()> {
        self.save_manifest_to_disk(manifest).await?;
        
        // Update in-memory manifest
        *self.manifest.write().await = Some(manifest.clone());
        
        Ok(())
    }
    
    /// Mark manifest as dirty and potentially save it based on batching interval
    async fn mark_manifest_dirty(&self) -> Result<()> {
        *self.manifest_dirty.write().await = true;
        
        let mut ops_count = self.operations_since_save.write().await;
        *ops_count += 1;
        
        // Save manifest every N operations for crash safety vs performance balance
        if *ops_count >= MANIFEST_SAVE_INTERVAL {
            drop(ops_count); // Release lock before calling save
            self.flush_manifest_if_dirty().await?;
        }
        
        Ok(())
    }
    
    /// Save manifest to disk if it's been modified, reset dirty flag
    async fn flush_manifest_if_dirty(&self) -> Result<()> {
        let is_dirty = {
            let dirty = self.manifest_dirty.read().await;
            *dirty
        };
        
        if is_dirty {
            let manifest = {
                let manifest_guard = self.manifest.read().await;
                manifest_guard.clone()
            };
            
            if let Some(manifest) = manifest {
                self.save_manifest_to_disk(&manifest).await?;
                *self.manifest_dirty.write().await = false;
                *self.operations_since_save.write().await = 0;
            }
        }
        
        Ok(())
    }
    
    async fn get_next_vector_offset(&self, vector_size: usize) -> Result<u64> {
        let current_offset = {
            let mut manifest_guard = self.manifest.write().await;
            if let Some(ref mut manifest) = *manifest_guard {
                let current_offset = manifest.next_vector_offset;
                let record_size = VECTOR_HEADER_SIZE + (vector_size * 4); // header + 4 bytes per f32
                manifest.next_vector_offset += record_size as u64;
                manifest.vector_file_size = manifest.next_vector_offset;
                current_offset
            } else {
                return Err(VectraError::StorageError {
                    message: "Manifest not initialized".to_string()
                });
            }
        };
        
        // Mark manifest as dirty for batched saving
        self.mark_manifest_dirty().await?;
        Ok(current_offset)
    }
    
    /// Ensure all pending changes are flushed to disk
    pub async fn flush(&self) -> Result<()> {
        // Flush manifest
        self.flush_manifest_if_dirty().await?;
        
        // Flush memory-mapped file if it exists
        if let Some(ref mmap) = *self.vector_mmap.read().await {
            mmap.flush()?;
        }
        
        // Flush RocksDB
        if let Some(ref db) = *self.db.read().await {
            db.flush()?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl crate::StorageBackend for OptimizedStorage {
    async fn exists(&self) -> bool {
        self.manifest_path().exists()
    }
    
    async fn create_index(&mut self, config: &CreateIndexConfig) -> Result<()> {
        let manifest_path = self.manifest_path();
        
        if manifest_path.exists() && !config.delete_if_exists {
            return Err(VectraError::IndexAlreadyExists { 
                path: manifest_path.to_string_lossy().to_string() 
            });
        }
        
        // Clean up existing files if delete_if_exists is true
        if config.delete_if_exists && self.path.exists() {
            fs::remove_dir_all(&self.path).await.ok();
        }
        
        let manifest = Manifest {
            version: 2,
            format: "optimized".to_string(),
            created_at: chrono::Utc::now(),
            dimensions: None,
            distance_metric: config.distance_metric.clone(),
            total_items: 0,
            vector_file_size: 0,
            next_vector_offset: 0,
        };
        
        self.save_manifest(&manifest).await?;
        
        // Initialize storage components
        self.initialize_storage().await?;
        
        // Make sure manifest is persisted for create operations
        self.flush_manifest_if_dirty().await?;
        
        // Create initial vector file (1MB initial size)
        self.create_vector_file(1024 * 1024).await?;
        
        Ok(())
    }
    
    async fn get_item(&self, id: &Uuid) -> Result<Option<VectorItem>> {
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let metadata_cf = db.cf_handle(METADATA_CF).unwrap();
            let vector_index_cf = db.cf_handle(VECTOR_INDEX_CF).unwrap();
            
            let id_bytes = id.as_bytes();
            
            // Get metadata
            if let Some(metadata_bytes) = db.get_cf(metadata_cf, id_bytes)? {
                let mut item: VectorItem = serde_json::from_slice(&metadata_bytes)?;
                
                // Get vector record
                if let Some(vector_record_bytes) = db.get_cf(vector_index_cf, id_bytes)? {
                    let vector_record: VectorRecord = bincode::deserialize(&vector_record_bytes)?;
                    
                    if !vector_record.deleted {
                        // Read vector from memory-mapped file
                        item.vector = self.read_vector_from_file(vector_record.offset, vector_record.dimensions).await?;
                        return Ok(Some(item));
                    }
                }
            }
        }
        
        Ok(None)
    }
    
    async fn insert_item(&mut self, item: &VectorItem) -> Result<()> {
        // Ensure storage is initialized
        if self.db.read().await.is_none() {
            self.initialize_storage().await?;
        }
        
        let dimensions = item.vector.len();
        
        // Set dimensions if this is the first item
        let mut needs_manifest_update = false;
        {
            let mut dims_guard = self.dimensions.write().await;
            if dims_guard.is_none() {
                *dims_guard = Some(dimensions);
                needs_manifest_update = true;
            } else if let Some(existing_dims) = *dims_guard {
                if existing_dims != dimensions {
                    return Err(VectraError::VectorValidation {
                        message: format!("Vector dimension mismatch: expected {}, got {}", existing_dims, dimensions)
                    });
                }
            }
        }
        
        // Update manifest outside of the dimensions lock to avoid deadlock
        if needs_manifest_update {
            let mut manifest_guard = self.manifest.write().await;
            if let Some(ref mut manifest) = *manifest_guard {
                manifest.dimensions = Some(dimensions);
                // Save manifest to disk only, we already have the in-memory version updated
                self.save_manifest_to_disk(manifest).await?;
            }
        }
        
        // Get offset for vector storage
        let vector_offset = self.get_next_vector_offset(dimensions).await?;
        
        // Write vector to memory-mapped file
        self.write_vector_to_file(&item.vector, vector_offset).await?;
        
        // Store metadata and vector record in RocksDB
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let metadata_cf = db.cf_handle(METADATA_CF).unwrap();
            let vector_index_cf = db.cf_handle(VECTOR_INDEX_CF).unwrap();
            
            let id_bytes = item.id.as_bytes();
            
            // Store metadata (without vector data) using JSON to handle serde_json::Value
            let mut metadata_item = item.clone();
            metadata_item.vector = Vec::new(); // Don't store vector in metadata
            let metadata_bytes = serde_json::to_vec(&metadata_item)?;
            db.put_cf(metadata_cf, id_bytes, metadata_bytes)?;
            
            // Store vector record
            let vector_record = VectorRecord {
                id: item.id,
                offset: vector_offset,
                dimensions,
                deleted: false,
            };
            let vector_record_bytes = bincode::serialize(&vector_record)?;
            db.put_cf(vector_index_cf, id_bytes, vector_record_bytes)?;
            
            // Update manifest
            {
                let mut manifest_guard = self.manifest.write().await;
                if let Some(ref mut manifest) = *manifest_guard {
                    manifest.total_items += 1;
                } else {
                    return Err(VectraError::StorageError {
                        message: "Manifest not initialized".to_string()
                    });
                }
            }
            
            // Mark manifest dirty for batched saving
            self.mark_manifest_dirty().await?;
        }
        
        Ok(())
    }
    
    async fn insert_items(&mut self, items: &[VectorItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        
        // Ensure storage is initialized
        if self.db.read().await.is_none() {
            self.initialize_storage().await?;
        }
        
        // Validate all items have same dimensions
        let first_dimensions = items[0].vector.len();
        for item in items {
            if item.vector.len() != first_dimensions {
                return Err(VectraError::VectorValidation {
                    message: format!("All vectors must have same dimensions. Expected {}, got {}", first_dimensions, item.vector.len())
                });
            }
        }
        
        // Set dimensions if this is the first batch
        let mut needs_manifest_update = false;
        {
            let mut dims_guard = self.dimensions.write().await;
            if dims_guard.is_none() {
                *dims_guard = Some(first_dimensions);
                needs_manifest_update = true;
            } else if let Some(existing_dims) = *dims_guard {
                if existing_dims != first_dimensions {
                    return Err(VectraError::VectorValidation {
                        message: format!("Vector dimension mismatch: expected {}, got {}", existing_dims, first_dimensions)
                    });
                }
            }
        }
        
        // Update manifest dimensions if needed
        if needs_manifest_update {
            let mut manifest_guard = self.manifest.write().await;
            if let Some(ref mut manifest) = *manifest_guard {
                manifest.dimensions = Some(first_dimensions);
                self.save_manifest_to_disk(manifest).await?;
            }
        }
        
        // Pre-allocate vector offsets and prepare data
        let mut prepared_data = Vec::with_capacity(items.len());
        for item in items {
            let vector_offset = self.get_next_vector_offset(first_dimensions).await?;
            self.write_vector_to_file(&item.vector, vector_offset).await?;
            
            // Prepare metadata (without vector data) using JSON
            let mut metadata_item = item.clone();
            metadata_item.vector = Vec::new();
            let metadata_bytes = serde_json::to_vec(&metadata_item)?;
            
            // Prepare vector record
            let vector_record = VectorRecord {
                id: item.id,
                offset: vector_offset,
                dimensions: first_dimensions,
                deleted: false,
            };
            let vector_record_bytes = bincode::serialize(&vector_record)?;
            
            prepared_data.push((item.id.as_bytes().to_vec(), metadata_bytes, vector_record_bytes));
        }
        
        // Bulk write to database
        let total_items_added = prepared_data.len();
        {
            let db_guard = self.db.read().await;
            if let Some(ref db) = *db_guard {
                let metadata_cf = db.cf_handle(METADATA_CF).unwrap();
                let vector_index_cf = db.cf_handle(VECTOR_INDEX_CF).unwrap();
                
                // Use RocksDB write batch for better performance
                let mut batch = rocksdb::WriteBatch::default();
                
                for (id_bytes, metadata_bytes, vector_record_bytes) in prepared_data {
                    batch.put_cf(metadata_cf, &id_bytes, metadata_bytes);
                    batch.put_cf(vector_index_cf, &id_bytes, vector_record_bytes);
                }
                
                // Execute batch write
                db.write(batch)?;
            }
        }
        
        // Update manifest
        {
            let mut manifest_guard = self.manifest.write().await;
            if let Some(ref mut manifest) = *manifest_guard {
                manifest.total_items += total_items_added;
            }
        }
        
        // Mark manifest dirty for batched saving
        self.mark_manifest_dirty().await?;
        
        Ok(())
    }
    
    async fn update_item(&mut self, item: &VectorItem) -> Result<()> {
        // For now, implement as delete + insert
        self.delete_item(&item.id).await?;
        self.insert_item(item).await?;
        Ok(())
    }
    
    async fn delete_item(&mut self, id: &Uuid) -> Result<()> {
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            let metadata_cf = db.cf_handle(METADATA_CF).unwrap();
            let vector_index_cf = db.cf_handle(VECTOR_INDEX_CF).unwrap();
            
            let id_bytes = id.as_bytes();
            
            // Mark vector record as deleted (we don't actually remove from file for now)
            if let Some(vector_record_bytes) = db.get_cf(vector_index_cf, id_bytes)? {
                let mut vector_record: VectorRecord = bincode::deserialize(&vector_record_bytes)?;
                vector_record.deleted = true;
                let updated_bytes = bincode::serialize(&vector_record)?;
                db.put_cf(vector_index_cf, id_bytes, updated_bytes)?;
            }
            
            // Remove metadata
            db.delete_cf(metadata_cf, id_bytes)?;
            
            // Update manifest
            let should_mark_dirty = {
                let mut manifest_guard = self.manifest.write().await;
                if let Some(ref mut manifest) = *manifest_guard {
                    if manifest.total_items > 0 {
                        manifest.total_items -= 1;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            
            // Mark manifest dirty for batched saving if updated
            if should_mark_dirty {
                self.mark_manifest_dirty().await?;
            }
        }
        
        Ok(())
    }
    
    async fn list_items(&self, options: Option<ListOptions>) -> Result<Vec<VectorItem>> {
        // Collect all the metadata records first without holding DB references
        let metadata_records = {
            let db_guard = self.db.read().await;
            if let Some(ref db) = *db_guard {
                let metadata_cf = db.cf_handle(METADATA_CF).unwrap();
                let vector_index_cf = db.cf_handle(VECTOR_INDEX_CF).unwrap();
                
                let mut records = Vec::new();
                let iter = db.iterator_cf(metadata_cf, rocksdb::IteratorMode::Start);
                
                for item in iter {
                    let (key, value) = item?;
                    
                    // Check if item is not deleted
                    if let Some(vector_record_bytes) = db.get_cf(vector_index_cf, &key)? {
                        let vector_record: VectorRecord = bincode::deserialize(&vector_record_bytes)?;
                        
                        if !vector_record.deleted {
                            let metadata_item: VectorItem = serde_json::from_slice(&value)?;
                            records.push((metadata_item, vector_record));
                            
                            // Apply limit if specified
                            if let Some(ref opts) = options {
                                if let Some(limit) = opts.limit {
                                    if records.len() >= limit {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                
                records
            } else {
                Vec::new()
            }
        };
        
        // Now load vectors without holding DB guard
        let mut items = Vec::new();
        for (mut metadata_item, vector_record) in metadata_records {
            // Load vector from file
            metadata_item.vector = self.read_vector_from_file(vector_record.offset, vector_record.dimensions).await?;
            items.push(metadata_item);
        }
        
        Ok(items)
    }
    
    async fn query_items(&self, query: &Query) -> Result<Vec<QueryResult>> {
        if let Some(ref query_vector) = query.vector {
            // For now, implement basic brute-force similarity search
            // In a real implementation, this would use HNSW index
            let all_items = self.list_items(None).await?;
            let mut results = Vec::new();
            
            for item in all_items {
                if item.vector.len() == query_vector.len() {
                    let similarity = VectorOps::cosine_similarity(query_vector, &item.vector);
                    results.push(QueryResult {
                        item,
                        score: similarity,
                    });
                }
            }
            
            // Sort by similarity (descending)
            results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
            
            // Apply limit
            results.truncate(query.top_k);
            
            Ok(results)
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn begin_transaction(&mut self) -> Result<()> {
        // For simplicity, we'll use RocksDB's default transaction behavior
        // In a full implementation, we'd use RocksDB transactions
        Ok(())
    }
    
    async fn commit_transaction(&mut self) -> Result<()> {
        // Flush any pending writes
        let db_guard = self.db.read().await;
        if let Some(ref db) = *db_guard {
            db.flush()?;
        }
        
        if let Some(ref mut mmap_guard) = *self.vector_mmap.write().await {
            mmap_guard.flush()?;
        }
        
        Ok(())
    }
    
    async fn rollback_transaction(&mut self) -> Result<()> {
        // In a full implementation, we'd rollback pending changes
        // For now, just return success
        Ok(())
    }
    
    async fn delete_index(&mut self) -> Result<()> {
        // Close database and memory map first
        *self.db.write().await = None;
        *self.vector_file.write().await = None;
        *self.vector_mmap.write().await = None;
        *self.manifest.write().await = None;
        
        // Remove all files in the index directory
        if self.path.exists() {
            fs::remove_dir_all(&self.path).await?;
        }
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<IndexStats> {
        if let Some(manifest) = self.load_manifest().await? {
            let size = if self.path.exists() {
                // Calculate directory size
                let mut total_size = 0u64;
                let mut entries = fs::read_dir(&self.path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    if let Ok(metadata) = entry.metadata().await {
                        total_size += metadata.len();
                    }
                }
                total_size
            } else {
                0
            };
            
            Ok(IndexStats {
                items: manifest.total_items,
                size,
                dimensions: manifest.dimensions,
                distance_metric: manifest.distance_metric,
            })
        } else {
            Ok(IndexStats {
                items: 0,
                size: 0,
                dimensions: None,
                distance_metric: DistanceMetric::Cosine,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StorageBackend;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_optimized_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = OptimizedStorage::new(temp_dir.path()).unwrap();
        
        assert!(!storage.exists().await);
        
        let config = CreateIndexConfig::default();
        storage.create_index(&config).await.unwrap();
        
        assert!(storage.exists().await);
    }
    
    #[tokio::test]
    async fn test_optimized_storage_insert_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = OptimizedStorage::new(temp_dir.path()).unwrap();
        
        let config = CreateIndexConfig::default();
        storage.create_index(&config).await.unwrap();
        
        let item = VectorItem {
            id: Uuid::new_v4(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({"test": "data"}),
            ..Default::default()
        };
        
        storage.insert_item(&item).await.unwrap();
        
        let retrieved = storage.get_item(&item.id).await.unwrap();
        assert!(retrieved.is_some());
        
        let retrieved_item = retrieved.unwrap();
        assert_eq!(retrieved_item.id, item.id);
        assert_eq!(retrieved_item.vector, item.vector);
    }
    
    #[tokio::test]
    async fn test_optimized_storage_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = OptimizedStorage::new(temp_dir.path()).unwrap();
        
        let config = CreateIndexConfig::default();
        storage.create_index(&config).await.unwrap();
        
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
        
        storage.insert_item(&item1).await.unwrap();
        storage.insert_item(&item2).await.unwrap();
        
        let query = Query {
            vector: Some(vec![1.0, 0.1, 0.0]),
            text: None,
            top_k: 2,
            filter: None,
        };
        
        let results = storage.query_items(&query).await.unwrap();
        assert_eq!(results.len(), 2);
        
        // First result should be more similar to item1
        assert_eq!(results[0].item.id, item1.id);
        assert!(results[0].score > results[1].score);
    }
}