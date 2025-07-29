// Hybrid search combining vector and text search
// TODO: Implement hybrid search

use vectrust_core::*;

pub struct HybridSearch;

impl HybridSearch {
    pub fn search(_query: &Query) -> Result<Vec<QueryResult>> {
        // TODO: Implement hybrid vector + text search
        Ok(Vec::new())
    }
}