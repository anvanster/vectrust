pub mod backend;
pub mod legacy;
pub mod optimized;
pub mod wal;
pub mod lock;

pub use backend::*;
pub use legacy::*;
pub use optimized::*;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_storage_auto_detect_new_index() {
        let temp_dir = TempDir::new().unwrap();
        let result = Storage::auto_detect(temp_dir.path(), "index.json");
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_legacy_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let result = LegacyStorage::new(temp_dir.path(), "index.json");
        assert!(result.is_ok());
    }
}