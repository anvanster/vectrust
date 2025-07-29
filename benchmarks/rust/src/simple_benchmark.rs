use anyhow::Result;
use std::time::Instant;
use tempfile::TempDir;
use vectrust::*;
use uuid::Uuid;
use serde_json::json;

pub async fn run_simple_benchmark() -> Result<()> {
    println!("🦀 Simple Vectra Rust Benchmark");
    println!("===============================");
    
    // Test basic functionality
    let temp_dir = TempDir::new()?;
    let index = LocalIndex::new(temp_dir.path(), None)?;
    
    // Create index
    let start = Instant::now();
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;
    let creation_time = start.elapsed();
    println!("✅ Index creation: {:.3}ms", creation_time.as_secs_f64() * 1000.0);
    
    // Insert some items
    let start = Instant::now();
    let items = vec![
        VectorItem {
            id: Uuid::new_v4(),
            vector: vec![1.0, 0.0, 0.0],
            metadata: json!({"text": "apple", "category": "fruit"}),
            ..Default::default()
        },
        VectorItem {
            id: Uuid::new_v4(),
            vector: vec![0.0, 1.0, 0.0],
            metadata: json!({"text": "banana", "category": "fruit"}),
            ..Default::default()
        },
        VectorItem {
            id: Uuid::new_v4(),
            vector: vec![0.0, 0.0, 1.0],
            metadata: json!({"text": "carrot", "category": "vegetable"}),
            ..Default::default()
        },
    ];
    
    for item in &items {
        index.insert_item(item.clone()).await?;
    }
    let insert_time = start.elapsed();
    println!("✅ Insert 3 items: {:.3}ms", insert_time.as_secs_f64() * 1000.0);
    
    // Query
    let start = Instant::now();
    let query_vector = vec![1.0, 0.1, 0.0];
    let results = index.query_items(query_vector, Some(3), None).await?;
    let query_time = start.elapsed();
    println!("✅ Query (top 3): {:.3}ms", query_time.as_secs_f64() * 1000.0);
    println!("   Found {} results", results.len());
    
    // Get item
    let start = Instant::now();
    let retrieved = index.get_item(&items[0].id).await?;
    let get_time = start.elapsed();
    println!("✅ Get item by ID: {:.3}ms", get_time.as_secs_f64() * 1000.0);
    println!("   Retrieved: {}", retrieved.is_some());
    
    // List items
    let start = Instant::now();
    let all_items = index.list_items(None).await?;
    let list_time = start.elapsed();
    println!("✅ List all items: {:.3}ms", list_time.as_secs_f64() * 1000.0);
    println!("   Total items: {}", all_items.len());
    
    // Stats
    let start = Instant::now();
    let stats = index.get_stats().await?;
    let stats_time = start.elapsed();
    println!("✅ Get stats: {:.3}ms", stats_time.as_secs_f64() * 1000.0);
    println!("   Items: {}, Dimensions: {:?}", stats.items, stats.dimensions);
    
    println!();
    println!("🎉 All operations completed successfully!");
    
    Ok(())
}