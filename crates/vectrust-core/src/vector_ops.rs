use crate::*;

/// Vector similarity calculations optimized for different distance metrics
pub struct VectorOps;

impl VectorOps {
    /// Calculate cosine similarity between two vectors
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        
        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;
        
        // Vectorized computation - compiler will auto-vectorize this loop
        for i in 0..a.len() {
            dot_product += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }
        
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        
        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }
    
    /// Calculate Euclidean distance between two vectors
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }
        
        let mut sum_sq = 0.0;
        for i in 0..a.len() {
            let diff = a[i] - b[i];
            sum_sq += diff * diff;
        }
        
        sum_sq.sqrt()
    }
    
    /// Calculate dot product between two vectors
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }
        
        let mut product = 0.0;
        for i in 0..a.len() {
            product += a[i] * b[i];
        }
        
        product
    }
    
    /// Calculate similarity based on the specified distance metric
    pub fn calculate_similarity(a: &[f32], b: &[f32], metric: &DistanceMetric) -> f32 {
        match metric {
            DistanceMetric::Cosine => Self::cosine_similarity(a, b),
            DistanceMetric::Euclidean => {
                // Convert distance to similarity (higher is better)
                let distance = Self::euclidean_distance(a, b);
                if distance == 0.0 {
                    1.0
                } else {
                    1.0 / (1.0 + distance)
                }
            },
            DistanceMetric::DotProduct => Self::dot_product(a, b),
        }
    }
    
    /// Normalize a vector to unit length
    pub fn normalize(vector: &mut [f32]) {
        let norm = vector.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in vector.iter_mut() {
                *x /= norm;
            }
        }
    }
    
    /// Create a normalized copy of a vector
    pub fn normalized(vector: &[f32]) -> Vec<f32> {
        let mut result = vector.to_vec();
        Self::normalize(&mut result);
        result
    }
    
    /// Check if two vectors have the same dimensions
    pub fn compatible_dimensions(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && !a.is_empty()
    }
    
    /// Validate vector for NaN or infinite values
    pub fn is_valid_vector(vector: &[f32]) -> bool {
        vector.iter().all(|&x| x.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((VectorOps::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
        
        let c = vec![0.0, 1.0, 0.0];
        assert!((VectorOps::cosine_similarity(&a, &c) - 0.0).abs() < 1e-6);
    }
    
    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        assert!((VectorOps::euclidean_distance(&a, &b) - 5.0).abs() < 1e-6);
    }
    
    #[test]
    fn test_normalization() {
        let mut vector = vec![3.0, 4.0, 0.0];
        VectorOps::normalize(&mut vector);
        let norm = vector.iter().map(|&x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }
}