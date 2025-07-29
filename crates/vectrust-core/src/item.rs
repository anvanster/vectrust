use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Maintains exact compatibility with Node.js VectorItem structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorItem {
    pub id: Uuid,
    pub vector: Vec<f32>,
    pub metadata: serde_json::Value,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexed: Option<serde_json::Value>,
    
    #[serde(default)]
    pub deleted: bool,
    
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
    
    #[serde(default)]
    pub version: u32,
}

impl Default for VectorItem {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            vector: Vec::new(),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            indexed: None,
            deleted: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRequest {
    pub id: Uuid,
    pub vector: Option<Vec<f32>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    pub id: Uuid,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub filter: Option<serde_json::Value>,
}