use rocksdb::{DB, Options};
use std::time::Instant;
use anyhow::Result;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Testing RocksDB performance in isolation");
    println!("==========================================");
    
    // Create a test database
    let temp_dir = tempfile::TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");
    
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    db_opts.create_missing_column_families(true);
    
    let cf_names = vec!["metadata", "vector_index"];
    let db = DB::open_cf(&db_opts, db_path, cf_names)?;
    
    let metadata_cf = db.cf_handle("metadata").unwrap();
    let vector_cf = db.cf_handle("vector_index").unwrap();
    
    println!("\n📊 Test 1: Sequential UUID keys");
    let mut times = Vec::new();
    
    for i in 0..500 {
        let id = Uuid::new_v4();
        let id_bytes = id.as_bytes();
        let value = format!("test_value_{}", i).into_bytes();
        
        let start = Instant::now();
        db.put_cf(metadata_cf, id_bytes, &value)?;
        db.put_cf(vector_cf, id_bytes, &value)?;
        times.push(start.elapsed().as_micros());
        
        if i % 100 == 99 {
            let avg = times[times.len()-100..].iter().sum::<u128>() / 100;
            println!("  Items {}-{}: {} µs/op", i-99, i, avg);
        }
    }
    
    let first_100_avg = times[..100].iter().sum::<u128>() / 100;
    let last_100_avg = times[times.len()-100..].iter().sum::<u128>() / 100;
    println!("  Degradation: {:.1}x", last_100_avg as f64 / first_100_avg as f64);
    
    println!("\n📊 Test 2: With WriteOptions (WAL disabled)");
    times.clear();
    
    let mut write_opts = rocksdb::WriteOptions::default();
    write_opts.disable_wal(true);
    
    for i in 0..500 {
        let id = Uuid::new_v4();
        let id_bytes = id.as_bytes();
        let value = format!("test_value_{}", i).into_bytes();
        
        let start = Instant::now();
        db.put_cf_opt(metadata_cf, id_bytes, &value, &write_opts)?;
        db.put_cf_opt(vector_cf, id_bytes, &value, &write_opts)?;
        times.push(start.elapsed().as_micros());
        
        if i % 100 == 99 {
            let avg = times[times.len()-100..].iter().sum::<u128>() / 100;
            println!("  Items {}-{}: {} µs/op", i-99, i, avg);
        }
    }
    
    let first_100_avg = times[..100].iter().sum::<u128>() / 100;
    let last_100_avg = times[times.len()-100..].iter().sum::<u128>() / 100;
    println!("  Degradation: {:.1}x", last_100_avg as f64 / first_100_avg as f64);
    
    println!("\n📊 Test 3: Sequential integer keys");
    times.clear();
    
    for i in 0..500u32 {
        let key = i.to_le_bytes();
        let value = format!("test_value_{}", i).into_bytes();
        
        let start = Instant::now();
        db.put_cf_opt(metadata_cf, &key, &value, &write_opts)?;
        db.put_cf_opt(vector_cf, &key, &value, &write_opts)?;
        times.push(start.elapsed().as_micros());
        
        if i % 100 == 99 {
            let avg = times[times.len()-100..].iter().sum::<u128>() / 100;
            println!("  Items {}-{}: {} µs/op", i-99, i, avg);
        }
    }
    
    let first_100_avg = times[..100].iter().sum::<u128>() / 100;
    let last_100_avg = times[times.len()-100..].iter().sum::<u128>() / 100;
    println!("  Degradation: {:.1}x", last_100_avg as f64 / first_100_avg as f64);
    
    Ok(())
}