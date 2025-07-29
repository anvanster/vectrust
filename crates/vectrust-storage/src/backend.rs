use std::path::Path;
use vectrust_core::*;

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