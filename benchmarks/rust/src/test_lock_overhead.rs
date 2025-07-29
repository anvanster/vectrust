use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Testing lock overhead pattern");
    println!("================================");
    
    // Simulate the vectrust pattern
    let manifest = Arc::new(RwLock::new(0u64));
    let vector_mmap = Arc::new(RwLock::new(vec![0u8; 10 * 1024 * 1024])); // 10MB
    
    let mut times = Vec::new();
    
    for i in 0..1000 {
        let start = Instant::now();
        
        // Simulate insert_item pattern:
        // 1. Get offset (write lock on manifest)
        let offset = {
            let mut m = manifest.write().await;
            let current = *m;
            *m += 520; // Simulate vector size
            current
        };
        
        // 2. Write vector (write lock on mmap)
        {
            let mut mmap = vector_mmap.write().await;
            // Simulate writing some data
            if offset as usize + 520 <= mmap.len() {
                mmap[offset as usize] = i as u8;
            }
        }
        
        // 3. Another manifest update (for dirty tracking)
        {
            let mut m = manifest.write().await;
            *m += 1; // Simulate operation count
        }
        
        times.push(start.elapsed().as_micros());
        
        if i % 200 == 199 {
            let avg = times[times.len()-200..].iter().sum::<u128>() / 200;
            println!("  Items {}-{}: {} µs/op", i-199, i, avg);
        }
    }
    
    let first_100_avg = times[..100].iter().sum::<u128>() / 100;
    let last_100_avg = times[times.len()-100..].iter().sum::<u128>() / 100;
    
    println!("\n📈 Lock overhead analysis:");
    println!("  First 100: {} µs/op", first_100_avg);
    println!("  Last 100:  {} µs/op", last_100_avg);
    println!("  Degradation: {:.1}x", last_100_avg as f64 / first_100_avg as f64);
    
    Ok(())
}