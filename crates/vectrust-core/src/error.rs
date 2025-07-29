use thiserror::Error;

pub type Result<T> = std::result::Result<T, VectraError>;

#[derive(Error, Debug)]
pub enum VectraError {
    #[error("Item not found")]
    ItemNotFound,
    
    #[error("Index not found at path: {path}")]
    IndexNotFound { path: String },
    
    #[error("Index already exists at path: {path}")]
    IndexAlreadyExists { path: String },
    
    #[error("Invalid vector dimensions: expected {expected}, got {actual}")]
    InvalidDimensions { expected: usize, actual: usize },
    
    #[error("Vector validation failed: {message}")]
    VectorValidation { message: String },
    
    #[error("Metadata validation failed: {message}")]
    MetadataValidation { message: String },
    
    #[error("Storage error: {message}")]
    Storage { message: String },
    
    #[error("Storage error: {message}")]
    StorageError { message: String },
    
    #[error("Lock error: {message}")]
    Lock { message: String },
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),
    
    #[error("Generic error: {0}")]
    Other(#[from] anyhow::Error),
}

impl From<rocksdb::Error> for VectraError {
    fn from(err: rocksdb::Error) -> Self {
        VectraError::StorageError {
            message: err.to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for VectraError {
    fn from(err: Box<bincode::ErrorKind>) -> Self {
        VectraError::StorageError {
            message: err.to_string(),
        }
    }
}