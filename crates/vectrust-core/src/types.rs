use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DistanceMetric {
    Cosine,
    Euclidean,
    DotProduct,
}

impl Default for DistanceMetric {
    fn default() -> Self {
        DistanceMetric::Cosine
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataConfig {
    #[serde(default)]
    pub indexed: Vec<String>,
    
    #[serde(default)]
    pub reserved: Vec<String>,
    
    #[serde(default = "default_max_size")]
    pub max_size: usize,
    
    #[serde(default = "default_dynamic")]
    pub dynamic: bool,
}

fn default_max_size() -> usize { 1048576 }
fn default_dynamic() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    #[serde(default = "default_m")]
    pub m: usize,
    
    #[serde(default = "default_ef_construction")]
    pub ef_construction: usize,
    
    #[serde(default = "default_ef_search")]
    pub ef_search: usize,
    
    #[serde(default)]
    pub random_seed: Option<u64>,
    
    #[serde(default = "default_max_elements")]
    pub max_elements: usize,
    
    #[serde(default = "default_max_levels")]
    pub max_levels: usize,
    
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    
    #[serde(default = "default_max_connections_layer0")]
    pub max_connections_layer0: usize,
    
    #[serde(default)]
    pub distance_metric: DistanceMetric,
}

fn default_m() -> usize { 16 }
fn default_ef_construction() -> usize { 200 }
fn default_ef_search() -> usize { 200 }
fn default_max_elements() -> usize { 10000 }
fn default_max_levels() -> usize { 16 }
fn default_max_connections() -> usize { 16 }
fn default_max_connections_layer0() -> usize { 32 }

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: default_m(),
            ef_construction: default_ef_construction(),
            ef_search: default_ef_search(),
            random_seed: None,
            max_elements: default_max_elements(),
            max_levels: default_max_levels(),
            max_connections: default_max_connections(),
            max_connections_layer0: default_max_connections_layer0(),
            distance_metric: DistanceMetric::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub items: usize,
    pub size: u64,
    pub dimensions: Option<usize>,
    pub distance_metric: DistanceMetric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub item: crate::VectorItem,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct Query {
    pub vector: Option<Vec<f32>>,
    pub text: Option<String>,
    pub top_k: usize,
    pub filter: Option<serde_json::Value>,
}