// HNSW (Hierarchical Navigable Small World) index implementation

use vectrust_core::*;
use std::collections::{HashMap, BinaryHeap, HashSet};
use std::cmp::Ordering;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct HnswNode {
    #[allow(dead_code)]
    id: Uuid,
    vector: Vec<f32>,
    #[allow(dead_code)]
    level: usize,
    connections: Vec<Vec<Uuid>>, // connections[level] = neighbors at that level
}

#[derive(Debug, Clone, PartialEq)]
struct SearchCandidate {
    id: Uuid,
    distance: f32,
}

impl Eq for SearchCandidate {}

impl Ord for SearchCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for SearchCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct HnswIndex {
    config: HnswConfig,
    nodes: HashMap<Uuid, HnswNode>,
    entry_point: Option<Uuid>,
    #[allow(dead_code)]
    level_multiplier: f64,
    max_level: usize,
}

impl HnswIndex {
    pub fn new(config: HnswConfig) -> Result<Self> {
        Ok(Self {
            config,
            nodes: HashMap::new(),
            entry_point: None,
            level_multiplier: 1.0 / (2.0_f64).ln(),
            max_level: 0,
        })
    }
    
    /// Generate random level for new node using exponential decay
    fn get_random_level(&self) -> usize {
        let mut level = 0;
        while level < self.config.max_levels && rand::random::<f64>() < 0.5 {
            level += 1;
        }
        level
    }
    
    /// Calculate distance between two vectors using configured metric
    /// Returns a distance value where smaller is better (closer)
    fn calculate_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.config.distance_metric {
            DistanceMetric::Cosine => 1.0 - VectorOps::cosine_similarity(a, b), // Convert similarity to distance
            DistanceMetric::Euclidean => VectorOps::euclidean_distance(a, b),
            DistanceMetric::DotProduct => -VectorOps::dot_product(a, b), // Convert dot product to distance (negate)
        }
    }
    
    /// Search for closest nodes at a specific level
    fn search_layer(&self, query: &[f32], entry_points: &[Uuid], num_closest: usize, level: usize) -> Vec<SearchCandidate> {
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut w = BinaryHeap::new(); // dynamic list of closest nodes
        
        // Initialize with entry points
        for &ep in entry_points {
            if let Some(node) = self.nodes.get(&ep) {
                let distance = self.calculate_distance(query, &node.vector);
                let candidate = SearchCandidate { id: ep, distance };
                candidates.push(candidate.clone());
                w.push(candidate);
                visited.insert(ep);
            }
        }
        
        while let Some(current) = candidates.pop() {
            // If current is farther than the farthest in w, stop
            if let Some(farthest) = w.iter().max_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap()) {
                if current.distance > farthest.distance && w.len() >= num_closest {
                    break;
                }
            }
            
            // Check neighbors of current node
            if let Some(node) = self.nodes.get(&current.id) {
                if level < node.connections.len() {
                    for &neighbor_id in &node.connections[level] {
                        if !visited.contains(&neighbor_id) {
                            visited.insert(neighbor_id);
                            
                            if let Some(neighbor) = self.nodes.get(&neighbor_id) {
                                let distance = self.calculate_distance(query, &neighbor.vector);
                                let candidate = SearchCandidate { id: neighbor_id, distance };
                                
                                // Add to candidates if better than worst in w
                                if w.len() < num_closest {
                                    candidates.push(candidate.clone());
                                    w.push(candidate);
                                } else if let Some(farthest) = w.iter().max_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap()) {
                                    if distance < farthest.distance {
                                        candidates.push(candidate.clone());
                                        w.push(candidate);
                                        
                                        // Remove farthest if w is too large
                                        if w.len() > num_closest {
                                            let mut temp_vec: Vec<_> = w.into_iter().collect();
                                            temp_vec.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
                                            temp_vec.truncate(num_closest);
                                            w = temp_vec.into_iter().collect();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        let mut result: Vec<_> = w.into_iter().collect();
        result.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        result.truncate(num_closest);
        result
    }
    
    /// Select diverse neighbors using heuristic
    fn select_neighbors(&self, candidates: Vec<SearchCandidate>, m: usize) -> Vec<Uuid> {
        if candidates.len() <= m {
            return candidates.into_iter().map(|c| c.id).collect();
        }
        
        let mut selected = Vec::new();
        let mut remaining = candidates;
        
        // Always select the closest
        if let Some(closest) = remaining.first() {
            selected.push(closest.id);
            remaining.remove(0);
        }
        
        // Select diverse neighbors
        while selected.len() < m && !remaining.is_empty() {
            let mut best_idx = 0;
            let mut best_score = f32::NEG_INFINITY;
            
            for (i, candidate) in remaining.iter().enumerate() {
                if let Some(candidate_node) = self.nodes.get(&candidate.id) {
                    // Calculate diversity score (distance to already selected)
                    let mut min_dist_to_selected = f32::INFINITY;
                    for &selected_id in &selected {
                        if let Some(selected_node) = self.nodes.get(&selected_id) {
                            let dist = self.calculate_distance(&candidate_node.vector, &selected_node.vector);
                            min_dist_to_selected = min_dist_to_selected.min(dist);
                        }
                    }
                    
                    // Score combines closeness to query and diversity
                    let score = min_dist_to_selected - candidate.distance;
                    if score > best_score {
                        best_score = score;
                        best_idx = i;
                    }
                }
            }
            
            let selected_candidate = remaining.remove(best_idx);
            selected.push(selected_candidate.id);
        }
        
        selected
    }
    
    pub fn insert(&mut self, id: Uuid, vector: &[f32]) -> Result<()> {
        let level = self.get_random_level();
        let mut node = HnswNode {
            id,
            vector: vector.to_vec(),
            level,
            connections: vec![Vec::new(); level + 1],
        };
        
        if self.entry_point.is_none() {
            // First node becomes entry point
            self.entry_point = Some(id);
            self.max_level = level;
            self.nodes.insert(id, node);
            return Ok(());
        }
        
        let entry_point = self.entry_point.unwrap();
        let mut current_closest = vec![entry_point];
        
        // Search from top level down to level+1
        for lc in (level + 1..=self.max_level).rev() {
            current_closest = self.search_layer(vector, &current_closest, 1, lc)
                .into_iter()
                .map(|c| c.id)
                .collect();
        }
        
        // Search and connect from level down to 0
        for lc in (0..=level).rev() {
            let candidates = self.search_layer(vector, &current_closest, self.config.ef_construction, lc);
            let m = if lc == 0 { self.config.max_connections } else { self.config.max_connections_layer0 };
            let selected_neighbors = self.select_neighbors(candidates.clone(), m);
            
            // Connect new node to selected neighbors
            node.connections[lc] = selected_neighbors.clone();
            
            // Connect selected neighbors back to new node
            for &neighbor_id in &selected_neighbors {
                // Add new node to neighbor's connections
                if let Some(neighbor) = self.nodes.get_mut(&neighbor_id) {
                    if lc < neighbor.connections.len() {
                        neighbor.connections[lc].push(id);
                    }
                }
            }
            
            // Prune connections in a separate pass to avoid borrowing conflicts
            for &neighbor_id in &selected_neighbors {
                if let Some(neighbor) = self.nodes.get(&neighbor_id) {
                    if lc < neighbor.connections.len() && neighbor.connections[lc].len() > m {
                        // Collect candidates for pruning
                        let neighbor_vector = neighbor.vector.clone();
                        let neighbor_connections = neighbor.connections[lc].clone();
                        
                        let neighbor_candidates: Vec<_> = neighbor_connections.iter()
                            .filter_map(|&nid| {
                                self.nodes.get(&nid).map(|n| SearchCandidate {
                                    id: nid,
                                    distance: self.calculate_distance(&neighbor_vector, &n.vector),
                                })
                            })
                            .collect();
                        
                        let pruned_neighbors = self.select_neighbors(neighbor_candidates, m);
                        
                        // Update the neighbor's connections
                        if let Some(neighbor_mut) = self.nodes.get_mut(&neighbor_id) {
                            if lc < neighbor_mut.connections.len() {
                                neighbor_mut.connections[lc] = pruned_neighbors;
                            }
                        }
                    }
                }
            }
            
            current_closest = candidates.into_iter().map(|c| c.id).collect();
        }
        
        // Update entry point if this node has higher level
        if level > self.max_level {
            self.entry_point = Some(id);
            self.max_level = level;
        }
        
        self.nodes.insert(id, node);
        Ok(())
    }
    
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(Uuid, f32)>> {
        if self.entry_point.is_none() {
            return Ok(Vec::new());
        }
        
        let entry_point = self.entry_point.unwrap();
        let mut current_closest = vec![entry_point];
        
        // Search from top level down to level 1
        for lc in (1..=self.max_level).rev() {
            current_closest = self.search_layer(query, &current_closest, 1, lc)
                .into_iter()
                .map(|c| c.id)
                .collect();
        }
        
        // Search level 0 with ef parameter
        let ef = self.config.ef_search.max(k);
        let candidates = self.search_layer(query, &current_closest, ef, 0);
        
        let mut results: Vec<_> = candidates.into_iter()
            .take(k)
            .map(|c| (c.id, c.distance))
            .collect();
        
        // Sort by distance (best first)
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        
        Ok(results)
    }
    
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

// Add rand dependency for random level generation
extern crate rand;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hnsw_creation() {
        let config = HnswConfig::default();
        let index = HnswIndex::new(config).unwrap();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }
    
    #[test]
    fn test_hnsw_insert_and_search() {
        let config = HnswConfig::default();
        let mut index = HnswIndex::new(config).unwrap();
        
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        
        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![0.0, 1.0, 0.0];  
        let vec3 = vec![0.0, 0.0, 1.0];
        
        index.insert(id1, &vec1).unwrap();
        index.insert(id2, &vec2).unwrap();
        index.insert(id3, &vec3).unwrap();
        
        assert_eq!(index.len(), 3);
        
        // Search for vector very similar to vec1
        let query = vec![0.99, 0.01, 0.0];
        let results = index.search(&query, 3).unwrap();
        
        assert_eq!(results.len(), 3);
        // Results should be sorted by distance (best first)
        // The first result should be the one closest to our query
        let closest_id = results[0].0;
        let closest_distance = results[0].1;
        
        // Verify that the closest result is indeed closest by checking distances manually
        let dist1 = 1.0 - VectorOps::cosine_similarity(&query, &vec1);
        let dist2 = 1.0 - VectorOps::cosine_similarity(&query, &vec2);
        let dist3 = 1.0 - VectorOps::cosine_similarity(&query, &vec3);
        
        let min_dist = dist1.min(dist2).min(dist3);
        assert!((closest_distance - min_dist).abs() < 0.001);
        
        // Since query is very close to vec1, id1 should be the closest
        if (dist1 - min_dist).abs() < 0.001 {
            assert_eq!(closest_id, id1);
        }
    }
}