use vectrust::*;
use anyhow::Result;
use std::time::Instant;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Isolating the performance issue");
    println!("==================================");
    
    // Test 1: Simple counter with lock contention
    println!("\n📊 Test 1: Lock contention");
    let counter = Arc::new(RwLock::new(0u64));
    let mut times = Vec::new();
    
    for i in 0..1000 {
        let start = Instant::now();
        {
            let mut guard = counter.write().await;
            *guard += 1;
        }
        times.push(start.elapsed().as_nanos());
        
        if i % 200 == 199 {
            let avg = times[times.len()-200..].iter().sum::<u128>() / 200;
            println!("  Items {}-{}: {} ns/op", i-199, i, avg);
        }
    }
    
    // Test 2: Batch operations
    println!("\n📊 Test 2: Batch lock operations");
    let counter2 = Arc::new(RwLock::new(0u64));
    let start = Instant::now();
    {
        let mut guard = counter2.write().await;
        for _ in 0..1000 {
            *guard += 1;
        }
    }
    println!("  1000 ops in single lock: {} µs total", start.elapsed().as_micros());
    
    // Test 3: Check if it's the manifest dirty tracking
    println!("\n📊 Test 3: Testing actual vectrust insert pattern");
    
    // Create index
    let temp_dir = tempfile::TempDir::new()?;
    let index = LocalIndex::new(temp_dir.path(), None)?;
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;
    
    // Insert items in batches
    for batch_num in 0..5 {
        let mut items = Vec::new();
        for i in 0..100 {
            items.push(VectorItem {
                id: uuid::Uuid::new_v4(),
                vector: vec![i as f32; 32],
                metadata: serde_json::json!({ "batch": batch_num, "i": i }),
                ..Default::default()
            });
        }
        
        let start = Instant::now();
        index.insert_items(items).await?;
        println!("  Batch {} (100 items): {} ms", batch_num, start.elapsed().as_millis());
    }
    
    Ok(())
}