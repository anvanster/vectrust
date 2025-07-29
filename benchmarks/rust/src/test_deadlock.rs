use vectrust::*;
use anyhow::Result;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔒 Testing for deadlock issues");
    println!("==============================");
    
    // Create small test data
    let dimensions = 128;
    let item_count = 100; // Start with fewer items
    
    println!("📊 Creating {} test vectors with {} dimensions", item_count, dimensions);
    let mut items = Vec::new();
    for i in 0..item_count {
        let mut vector = vec![0.0; dimensions];
        vector[0] = i as f32;
        
        items.push(VectorItem {
            id: uuid::Uuid::new_v4(),
            vector,
            metadata: serde_json::json!({ "index": i }),
            ..Default::default()
        });
    }
    
    println!("🏗️  Creating index...");
    let temp_dir = tempfile::TempDir::new()?;
    let index = LocalIndex::new(temp_dir.path(), None)?;
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;
    
    // Test with smaller batches first
    println!("\n📥 Testing with 10 items...");
    let small_batch = items[0..10].to_vec();
    match timeout(Duration::from_secs(5), index.insert_items(small_batch)).await {
        Ok(Ok(_)) => println!("✅ 10 items inserted successfully"),
        Ok(Err(e)) => println!("❌ Error inserting 10 items: {:?}", e),
        Err(_) => println!("⏱️  Timeout after 5 seconds!"),
    }
    
    println!("\n📥 Testing with 50 items...");
    let medium_batch = items[0..50].to_vec();
    match timeout(Duration::from_secs(5), index.insert_items(medium_batch)).await {
        Ok(Ok(_)) => println!("✅ 50 items inserted successfully"),
        Ok(Err(e)) => println!("❌ Error inserting 50 items: {:?}", e),
        Err(_) => println!("⏱️  Timeout after 5 seconds!"),
    }
    
    println!("\n📥 Testing with 100 items...");
    match timeout(Duration::from_secs(5), index.insert_items(items)).await {
        Ok(Ok(_)) => println!("✅ 100 items inserted successfully"),
        Ok(Err(e)) => println!("❌ Error inserting 100 items: {:?}", e),
        Err(_) => println!("⏱️  Timeout after 5 seconds!"),
    }
    
    // Test individual inserts
    println!("\n📥 Testing individual inserts...");
    let test_item = VectorItem {
        id: uuid::Uuid::new_v4(),
        vector: vec![1.0; dimensions],
        metadata: serde_json::json!({ "test": true }),
        ..Default::default()
    };
    
    match timeout(Duration::from_secs(2), index.insert_item(test_item)).await {
        Ok(Ok(_)) => println!("✅ Single item inserted successfully"),
        Ok(Err(e)) => println!("❌ Error inserting single item: {:?}", e),
        Err(_) => println!("⏱️  Timeout after 2 seconds!"),
    }
    
    Ok(())
}