use vectrust::*;
use anyhow::Result;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Testing ONLY 3000 items multiple times");
    println!("=========================================");
    
    let dimensions = 128;
    let item_count = 3000;
    
    for run in 1..=3 {
        println!("\n📊 Run #{} - Testing with {} items", run, item_count);
        
        // Create fresh test data
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
        
        // Create fresh index
        let temp_dir = tempfile::TempDir::new()?;
        let index = LocalIndex::new(temp_dir.path(), None)?;
        let config = CreateIndexConfig::default();
        index.create_index(Some(config)).await?;
        
        let start = Instant::now();
        match index.insert_items(items).await {
            Ok(_) => {
                let elapsed = start.elapsed();
                println!("  ✅ Success in {:.2}s ({:.0} items/sec)", 
                         elapsed.as_secs_f64(), 
                         item_count as f64 / elapsed.as_secs_f64());
            }
            Err(e) => {
                println!("  ❌ Error: {:?}", e);
            }
        }
    }
    
    Ok(())
}