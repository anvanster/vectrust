use vectrust::*;
use anyhow::Result;
use std::time::{Duration, Instant};
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Testing batch sizes to find hang threshold");
    println!("============================================");
    
    let dimensions = 128;
    let test_sizes = vec![100, 500, 1000, 2000, 3000, 4000, 5000];
    
    for size in test_sizes {
        println!("\n📊 Testing with {} items...", size);
        
        // Create fresh test data for each test
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
        
        // Create fresh index for each test
        let temp_dir = tempfile::TempDir::new()?;
        let index = LocalIndex::new(temp_dir.path(), None)?;
        let config = CreateIndexConfig::default();
        index.create_index(Some(config)).await?;
        
        let start = Instant::now();
        match timeout(Duration::from_secs(30), index.insert_items(items)).await {
            Ok(Ok(_)) => {
                let elapsed = start.elapsed();
                println!("  ✅ Success in {:.2}s ({:.0} items/sec)", 
                         elapsed.as_secs_f64(), 
                         size as f64 / elapsed.as_secs_f64());
            }
            Ok(Err(e)) => {
                println!("  ❌ Error: {:?}", e);
            }
            Err(_) => {
                println!("  ⏱️  TIMEOUT after 30 seconds!");
                println!("  🚨 Found threshold: hangs at {} items", size);
                break;
            }
        }
    }
    
    Ok(())
}