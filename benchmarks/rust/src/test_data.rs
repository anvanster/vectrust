use rand::prelude::*;
use serde_json::json;
use uuid::Uuid;
use vectrust::*;

pub struct TestDataGenerator {
    dimensions: usize,
    rng: StdRng,
}

impl TestDataGenerator {
    pub fn new(dimensions: usize) -> Self {
        Self {
            dimensions,
            rng: StdRng::seed_from_u64(42), // Fixed seed for reproducible benchmarks
        }
    }
    
    pub fn generate_vectors(&mut self, count: usize) -> Vec<VectorItem> {
        (0..count)
            .map(|i| self.generate_vector(i))
            .collect()
    }
    
    pub fn generate_vector(&mut self, index: usize) -> VectorItem {
        let vector: Vec<f32> = (0..self.dimensions)
            .map(|_| self.rng.gen_range(-1.0..1.0))
            .collect();
        
        // Normalize the vector
        let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        let normalized_vector = if norm > 0.0 {
            vector.iter().map(|x| x / norm).collect()
        } else {
            vector
        };
        
        VectorItem {
            id: Uuid::new_v4(),
            vector: normalized_vector,
            metadata: self.generate_metadata(index),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            version: 1,
            deleted: false,
            indexed: Some(serde_json::json!(true)),
        }
    }
    
    fn generate_metadata(&mut self, index: usize) -> serde_json::Value {
        let categories = ["technology", "science", "art", "sports", "music", "travel", "food", "health"];
        let authors = ["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry"];
        
        json!({
            "title": format!("Document {}", index),
            "category": categories[index % categories.len()],
            "author": authors[self.rng.gen_range(0..authors.len())],
            "score": self.rng.gen_range(0.0..1.0),
            "tags": self.generate_tags(),
            "length": self.rng.gen_range(100..5000),
            "created": chrono::Utc::now().to_rfc3339(),
        })
    }
    
    fn generate_tags(&mut self) -> Vec<String> {
        let all_tags = [
            "important", "urgent", "draft", "published", "archived",
            "featured", "trending", "popular", "new", "updated",
            "experimental", "stable", "beta", "alpha", "deprecated"
        ];
        
        let num_tags = self.rng.gen_range(1..=5);
        let mut tags = Vec::new();
        
        for _ in 0..num_tags {
            let tag = all_tags[self.rng.gen_range(0..all_tags.len())];
            if !tags.contains(&tag.to_string()) {
                tags.push(tag.to_string());
            }
        }
        
        tags
    }
    
    /// Generate vectors with specific similarity patterns for testing
    #[allow(dead_code)]
    pub fn generate_clustered_vectors(&mut self, count: usize, num_clusters: usize) -> Vec<VectorItem> {
        let mut vectors = Vec::new();
        let cluster_size = count / num_clusters;
        
        for cluster_id in 0..num_clusters {
            // Generate a random cluster center
            let center: Vec<f32> = (0..self.dimensions)
                .map(|_| self.rng.gen_range(-1.0..1.0))
                .collect();
            
            // Generate vectors around this center
            for i in 0..cluster_size {
                let mut vector = Vec::new();
                
                for j in 0..self.dimensions {
                    // Add noise to the center
                    let noise = self.rng.gen_range(-0.3..0.3);
                    vector.push(center[j] + noise);
                }
                
                // Normalize
                let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    vector = vector.iter().map(|x| x / norm).collect();
                }
                
                vectors.push(VectorItem {
                    id: Uuid::new_v4(),
                    vector,
                    metadata: json!({
                        "cluster": cluster_id,
                        "cluster_member": i,
                        "title": format!("Cluster {} Item {}", cluster_id, i)
                    }),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    version: 1,
                    deleted: false,
                    indexed: Some(serde_json::json!(true)),
                });
            }
        }
        
        // Add remaining vectors to fill count
        while vectors.len() < count {
            vectors.push(self.generate_vector(vectors.len()));
        }
        
        vectors
    }
    
    /// Generate high-dimensional sparse vectors (many zeros)
    #[allow(dead_code)]
    pub fn generate_sparse_vectors(&mut self, count: usize, sparsity: f32) -> Vec<VectorItem> {
        (0..count)
            .map(|i| {
                let mut vector = vec![0.0; self.dimensions];
                let non_zero_count = ((self.dimensions as f32) * (1.0 - sparsity)) as usize;
                
                // Randomly select positions for non-zero values
                let mut positions: Vec<usize> = (0..self.dimensions).collect();
                positions.shuffle(&mut self.rng);
                
                for &pos in positions.iter().take(non_zero_count) {
                    vector[pos] = self.rng.gen_range(-1.0..1.0);
                }
                
                // Normalize
                let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    vector = vector.iter().map(|x| x / norm).collect();
                }
                
                VectorItem {
                    id: Uuid::new_v4(),
                    vector,
                    metadata: json!({
                        "type": "sparse",
                        "sparsity": sparsity,
                        "index": i
                    }),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                    version: 1,
                    deleted: false,
                    indexed: Some(serde_json::json!(true)),
                }
            })
            .collect()
    }
}