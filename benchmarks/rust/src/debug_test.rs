use vectrust::*;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🐛 Debug Test - Insert 5000 items");
    println!("==================================");
    
    // Create test data
    let dimensions = 128;
    let item_count = 5000;
    
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
    
    println!("📥 Starting bulk insert of {} items...", items.len());
    let start = std::time::Instant::now();
    
    match index.insert_items(items).await {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("✅ Bulk insert completed in {:.2} seconds", elapsed.as_secs_f64());
        }
        Err(e) => {
            println!("❌ Bulk insert failed: {:?}", e);
        }
    }
    
    Ok(())
}