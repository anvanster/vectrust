use std::time::Instant;
use anyhow::Result;
use memmap2::{MmapMut, MmapOptions};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Testing memory-mapped file performance");
    println!("========================================");
    
    // Create a test file
    let temp_dir = tempfile::TempDir::new()?;
    let file_path = temp_dir.path().join("test.dat");
    
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)?;
    
    // Pre-allocate 10MB
    file.seek(SeekFrom::Start(10 * 1024 * 1024 - 1))?;
    file.write_all(&[0])?;
    file.flush()?;
    
    let mut mmap = unsafe { MmapOptions::new().map_mut(&file)? };
    
    println!("\n📊 Test 1: Sequential writes to mmap");
    let mut times = Vec::new();
    let record_size = 520; // 128 dims * 4 bytes + 8 header
    
    for i in 0..500 {
        let offset = i * record_size;
        let data = vec![i as u8; record_size];
        
        let start = Instant::now();
        mmap[offset..offset + record_size].copy_from_slice(&data);
        times.push(start.elapsed().as_nanos());
        
        if i % 100 == 99 {
            let avg = times[times.len()-100..].iter().sum::<u128>() / 100;
            println!("  Items {}-{}: {} ns/op", i-99, i, avg);
        }
    }
    
    let first_100_avg = times[..100].iter().sum::<u128>() / 100;
    let last_100_avg = times[times.len()-100..].iter().sum::<u128>() / 100;
    println!("  Degradation: {:.1}x", last_100_avg as f64 / first_100_avg as f64);
    
    println!("\n📊 Test 2: With async RwLock pattern (like vectrust)");
    let mmap_arc = std::sync::Arc::new(tokio::sync::RwLock::new(mmap));
    times.clear();
    
    for i in 0..500 {
        let offset = i * record_size;
        let data = vec![i as u8; record_size];
        
        let start = Instant::now();
        {
            let mut guard = mmap_arc.write().await;
            guard[offset..offset + record_size].copy_from_slice(&data);
        }
        times.push(start.elapsed().as_nanos());
        
        if i % 100 == 99 {
            let avg = times[times.len()-100..].iter().sum::<u128>() / 100;
            println!("  Items {}-{}: {} ns/op", i-99, i, avg);
        }
    }
    
    let first_100_avg = times[..100].iter().sum::<u128>() / 100;
    let last_100_avg = times[times.len()-100..].iter().sum::<u128>() / 100;
    println!("  Degradation: {:.1}x", last_100_avg as f64 / first_100_avg as f64);
    
    Ok(())
}