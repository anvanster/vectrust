use napi_derive::napi;
use napi::{Result, Error};
use vectrust::{LocalIndex as RustLocalIndex, VectorItem, CreateIndexConfig, ListOptions};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

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
            let config: CreateIndexConfig = serde_json::from_str(&config_str)
                .map_err(|e| Error::from_reason(e.to_string()))?;
            Some(config)
        } else {
            None
        };
        
        let index = self.inner.lock().await;
        index.create_index(config).await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn is_index_created(&self) -> Result<bool> {
        let index = self.inner.lock().await;
        Ok(index.is_index_created().await)
    }
    
    #[napi]
    pub async fn insert_item(&self, item_json: String) -> Result<String> {
        let vector_item: VectorItem = serde_json::from_str(&item_json)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        let index = self.inner.lock().await;
        let result = index.insert_item(vector_item).await
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        serde_json::to_string(&result)
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn get_item(&self, id: String) -> Result<Option<String>> {
        let uuid = Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        let index = self.inner.lock().await;
        let result = index.get_item(&uuid).await
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        match result {
            Some(item) => {
                let json = serde_json::to_string(&item)
                    .map_err(|e| Error::from_reason(e.to_string()))?;
                Ok(Some(json))
            },
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
            let filter_value: serde_json::Value = serde_json::from_str(&filter_str)
                .map_err(|e| Error::from_reason(e.to_string()))?;
            Some(filter_value)
        } else {
            None
        };
        
        let index = self.inner.lock().await;
        let results = index.query_items(vector, top_k, filter).await
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        serde_json::to_string(&results)
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn delete_item(&self, id: String) -> Result<()> {
        let uuid = Uuid::parse_str(&id)
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        let index = self.inner.lock().await;
        index.delete_item(&uuid).await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn list_items(&self, options: Option<String>) -> Result<String> {
        let list_options = if let Some(opts_str) = options {
            let opts: ListOptions = serde_json::from_str(&opts_str)
                .map_err(|e| Error::from_reason(e.to_string()))?;
            Some(opts)
        } else {
            None
        };
        
        let index = self.inner.lock().await;
        let items = index.list_items(list_options).await
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        serde_json::to_string(&items)
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn begin_update(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index.begin_update().await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn end_update(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index.end_update().await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn cancel_update(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index.cancel_update().await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
    
    #[napi]
    pub async fn delete_index(&self) -> Result<()> {
        let index = self.inner.lock().await;
        index.delete_index().await
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}

