use vectrust::*;
use anyhow::Result;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    println!("📊 Profiling insert components");
    println!("==============================");
    
    let dimensions = 128;
    let test_sizes = vec![100, 500, 1000, 2000];
    
    for size in test_sizes {
        println!("\n📦 Testing with {} items", size);
        
        // Create test data
        let mut items = Vec::new();
        for i in 0..size {
            let mut vector = vec![0.0; dimensions];
            vector[0] = i as f32;
            
            items.push(VectorItem {
                id: uuid::Uuid::new_v4(),
                vector,
                metadata: serde_json::json!({ "index": i }),
                ..Default::default()
            });
        }
        
        // Create index
        let temp_dir = tempfile::TempDir::new()?;
        let index = LocalIndex::new(temp_dir.path(), None)?;
        let config = CreateIndexConfig::default();
        
        let start = Instant::now();
        index.create_index(Some(config)).await?;
        let create_time = start.elapsed();
        
        // Time the bulk insert
        let start = Instant::now();
        index.insert_items(items.clone()).await?;
        let insert_time = start.elapsed();
        
        // Time a query to see if read performance is affected
        let start = Instant::now();
        let _results = index.query_items(items[0].vector.clone(), Some(10), None).await?;
        let query_time = start.elapsed();
        
        println!("  Create index: {:.2} ms", create_time.as_secs_f64() * 1000.0);
        println!("  Insert items: {:.2} ms ({:.0} items/sec)", 
                 insert_time.as_secs_f64() * 1000.0,
                 size as f64 / insert_time.as_secs_f64());
        println!("  Query time:   {:.2} ms", query_time.as_secs_f64() * 1000.0);
    }
    
    Ok(())
}