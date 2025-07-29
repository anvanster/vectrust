use vectrust::*;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Testing insert with 3000 items to catch error");
    println!("================================================");
    
    let dimensions = 128;
    let item_count = 3000;
    
    // Create test data
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
    
    // Create index
    let temp_dir = tempfile::TempDir::new()?;
    let index = LocalIndex::new(temp_dir.path(), None)?;
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;
    
    // Try to insert and catch any error
    println!("📥 Starting bulk insert of {} items...", items.len());
    match index.insert_items(items).await {
        Ok(_) => {
            println!("✅ Bulk insert completed successfully!");
        }
        Err(e) => {
            println!("❌ Bulk insert failed with error: {:?}", e);
        }
    }
    
    Ok(())
}