use vectrust::*;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new index
    let index = LocalIndex::new("./example_index", None)?;
    
    // Create the index with default configuration
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;
    
    println!("Created index");
    
    // Insert some test items
    let items = vec![
        VectorItem {
            id: Uuid::new_v4(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({"text": "apple", "category": "fruit"}),
            ..Default::default()
        },
        VectorItem {
            id: Uuid::new_v4(),
            vector: vec![0.0, 1.0, 0.0],
            metadata: serde_json::json!({"text": "banana", "category": "fruit"}),
            ..Default::default()
        },
        VectorItem {
            id: Uuid::new_v4(),
            vector: vec![0.0, 0.0, 1.0],
            metadata: serde_json::json!({"text": "carrot", "category": "vegetable"}),
            ..Default::default()
        },
    ];
    
    for item in items {
        let inserted = index.insert_item(item).await?;
        println!("Inserted item: {}", inserted.id);
    }
    
    // Query for similar items
    let query_vector = vec![1.0, 0.1, 0.0]; // Should be most similar to apple
    let results = index.query_items(query_vector, Some(3), None).await?;
    
    println!("\nQuery results:");
    for result in results {
        println!("ID: {}, Score: {:.3}, Text: {}", 
                 result.item.id, 
                 result.score,
                 result.item.metadata["text"]);
    }
    
    // List all items
    let all_items = index.list_items(None).await?;
    println!("\nAll items: {}", all_items.len());
    
    // Get statistics
    let stats = index.get_stats().await?;
    println!("Index stats: {} items, dimensions: {:?}", 
             stats.items, stats.dimensions);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_basic_example_flow() {
        let temp_dir = TempDir::new().unwrap();
        let index = LocalIndex::new(temp_dir.path(), None).unwrap();
        
        // Test index creation
        let config = CreateIndexConfig::default();
        index.create_index(Some(config)).await.unwrap();
        assert!(index.is_index_created().await);
        
        // Test item insertion
        let item = VectorItem {
            id: Uuid::new_v4(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: serde_json::json!({"test": "data"}),
            ..Default::default()
        };
        
        let inserted = index.insert_item(item.clone()).await.unwrap();
        assert_eq!(inserted.id, item.id);
        
        // Test querying
        let results = index.query_items(vec![1.0, 0.0, 0.0], Some(1), None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.id, item.id);
    }
}