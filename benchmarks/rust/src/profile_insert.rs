use vectrust::*;
use anyhow::Result;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    println!("📊 Profiling insert performance");
    println!("===============================");
    
    let dimensions = 128;
    let item_count = 1000;
    
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
    
    // Insert items one by one with timing
    println!("\n📥 Inserting {} items individually with timing...", items.len());
    let mut timings = Vec::new();
    
    for (idx, item) in items.into_iter().enumerate() {
        let start = Instant::now();
        index.insert_item(item).await?;
        let elapsed = start.elapsed();
        timings.push(elapsed.as_micros());
        
        if idx % 100 == 0 {
            let avg_time = timings[timings.len().saturating_sub(100)..].iter().sum::<u128>() / 100;
            println!("  Item {}: last 100 avg = {} µs/item", idx, avg_time);
        }
    }
    
    // Analyze timings
    let first_100_avg = timings[..100].iter().sum::<u128>() / 100;
    let last_100_avg = timings[timings.len()-100..].iter().sum::<u128>() / 100;
    
    println!("\n📈 Performance Analysis:");
    println!("  First 100 items: {} µs/item", first_100_avg);
    println!("  Last 100 items:  {} µs/item", last_100_avg);
    println!("  Slowdown factor: {:.1}x", last_100_avg as f64 / first_100_avg as f64);
    
    Ok(())
}