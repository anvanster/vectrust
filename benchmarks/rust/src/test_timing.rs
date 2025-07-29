use vectrust::*;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("📊 Testing insert timing breakdown");
    println!("==================================");
    
    let dimensions = 32;
    let item_count = 1000;
    
    // Create test data
    let mut items = Vec::new();
    for i in 0..item_count {
        items.push(VectorItem {
            id: uuid::Uuid::new_v4(),
            vector: vec![i as f32; dimensions],
            metadata: serde_json::json!({ "i": i }),
            ..Default::default()
        });
    }
    
    // Create index
    let temp_dir = tempfile::TempDir::new()?;
    let index = LocalIndex::new(temp_dir.path(), None)?;
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;
    
    println!("\nInserting {} items individually to see timing breakdown...", item_count);
    
    for (i, item) in items.into_iter().enumerate() {
        index.insert_item(item).await?;
        if i % 100 == 99 {
            println!("  Inserted {} items", i + 1);
        }
    }
    
    println!("\nDone!");
    
    Ok(())
}