use vectrust::*;
use anyhow::Result;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Finding the bottleneck");
    println!("========================");
    
    // Test with simple dimensions to isolate the issue
    let dimensions = 32; // Smaller to reduce vector write overhead
    let test_sizes = vec![100, 200, 300, 400, 500];
    
    for size in test_sizes {
        println!("\n📊 Testing {} items (dimensions: {})", size, dimensions);
        
        // Create test data
        let mut items = Vec::new();
        for i in 0..size {
            items.push(VectorItem {
                id: uuid::Uuid::new_v4(),
                vector: vec![i as f32; dimensions],
                metadata: serde_json::json!({ "i": i }),
                ..Default::default()
            });
        }
        
        // Create fresh index
        let temp_dir = tempfile::TempDir::new()?;
        let index = LocalIndex::new(temp_dir.path(), None)?;
        let config = CreateIndexConfig::default();
        index.create_index(Some(config)).await?;
        
        // Time individual inserts
        let mut insert_times = Vec::new();
        for item in items {
            let start = Instant::now();
            index.insert_item(item).await?;
            insert_times.push(start.elapsed().as_micros());
        }
        
        // Analyze the pattern
        let first_10_avg = insert_times[..10].iter().sum::<u128>() / 10;
        let last_10_avg = insert_times[insert_times.len()-10..].iter().sum::<u128>() / 10;
        let total_time: u128 = insert_times.iter().sum();
        
        println!("  First 10 avg: {} µs", first_10_avg);
        println!("  Last 10 avg:  {} µs", last_10_avg);
        println!("  Total time:   {} ms", total_time / 1000);
        println!("  Degradation:  {:.1}x", last_10_avg as f64 / first_10_avg as f64);
    }
    
    Ok(())
}