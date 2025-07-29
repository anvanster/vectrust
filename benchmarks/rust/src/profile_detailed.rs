use vectrust::*;
use anyhow::Result;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    println!("📊 Detailed profiling of insert operations");
    println!("=========================================");
    
    let dimensions = 32;
    let item_count = 200;
    
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
    
    println!("\nInserting {} items with detailed timing...", item_count);
    
    // Warm up
    for i in 0..10 {
        let item = VectorItem {
            id: uuid::Uuid::new_v4(),
            vector: vec![999.0 + i as f32; dimensions],
            metadata: serde_json::json!({ "warmup": i }),
            ..Default::default()
        };
        index.insert_item(item).await?;
    }
    
    // Profile actual inserts
    let mut total_times = Vec::new();
    
    for (idx, item) in items.into_iter().enumerate() {
        let start = Instant::now();
        index.insert_item(item).await?;
        let total = start.elapsed();
        
        total_times.push(total.as_micros());
        
        // Print details every 50 items
        if idx % 50 == 49 {
            let recent_avg = total_times[total_times.len()-50..].iter().sum::<u128>() / 50;
            println!("  Items {}-{}: avg {} µs/op", idx-49, idx, recent_avg);
        }
    }
    
    // Final analysis
    let first_50_avg = total_times[..50].iter().sum::<u128>() / 50;
    let last_50_avg = total_times[total_times.len()-50..].iter().sum::<u128>() / 50;
    
    println!("\n📈 Final Analysis:");
    println!("  First 50 items: {} µs/op", first_50_avg);
    println!("  Last 50 items:  {} µs/op", last_50_avg);
    println!("  Degradation:    {:.1}x", last_50_avg as f64 / first_50_avg as f64);
    
    // Check manifest state
    let stats = index.get_stats().await?;
    println!("\n📊 Index Stats:");
    println!("  Total items: {}", stats.items);
    println!("  Index size:  {} KB", stats.size / 1024);
    
    Ok(())
}