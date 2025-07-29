use clap::Parser;
use anyhow::Result;
use std::path::PathBuf;
use std::time::Instant;
use serde_json::json;
use indicatif::{ProgressBar, ProgressStyle};
use vectrust::*;

mod test_data;
mod simple_benchmark;

use test_data::*;
use simple_benchmark::*;

#[derive(Parser)]
#[command(name = "vectra-benchmark")]
#[command(about = "Comprehensive Vectra Rust benchmarks")]
struct Args {
    /// Output directory for results
    #[arg(short, long, default_value = "../results")]
    output: PathBuf,
    
    /// Number of vectors to test with
    #[arg(short, long, default_value = "10000")]
    vectors: usize,
    
    /// Vector dimensions
    #[arg(short, long, default_value = "384")]
    dimensions: usize,
    
    /// Specific benchmark to run
    #[arg(short, long)]
    benchmark: Option<String>,
    
    /// Number of iterations for timing
    #[arg(short, long, default_value = "5")]
    iterations: usize,
    
    /// Use legacy storage format
    #[arg(long)]
    legacy: bool,
    
    /// Verbose output
    #[arg(long)]
    verbose: bool,
    
    /// Run simple benchmark instead
    #[arg(long)]
    simple: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    println!("🦀 Vectra Rust Benchmark Suite");
    println!("================================");
    println!("Vectors: {}", args.vectors);
    println!("Dimensions: {}", args.dimensions);
    println!("Iterations: {}", args.iterations);
    println!("Storage: {}", if args.legacy { "Legacy JSON" } else { "Optimized v2" });
    println!();
    
    // Check if simple benchmark requested
    if args.simple {
        run_simple_benchmark().await?;
        return Ok(());
    }
    
    // Setup test data
    let mut test_data = TestDataGenerator::new(args.dimensions);
    let vectors = test_data.generate_vectors(args.vectors);
    let query_vectors = test_data.generate_vectors(100);
    
    let mut results = BenchmarkResults::new();
    
    println!("🔧 Running comprehensive benchmarks...\n");
    
    // Index benchmarks
    run_index_benchmarks(&args, &vectors, &mut results).await?;
    
    // Item benchmarks  
    run_item_benchmarks(&args, &vectors, &mut results).await?;
    
    // Search benchmarks
    run_search_benchmarks(&args, &vectors, &query_vectors, &mut results).await?;
    
    // Scale benchmarks
    run_scale_benchmarks(&args, &vectors, &mut results).await?;
    
    println!();
    results.print_summary();
    results.save(&args.output).await?;
    
    println!("\n🎉 Benchmark suite completed successfully!");
    println!("Results saved to: {:?}", args.output);
    
    Ok(())
}

async fn run_index_benchmarks(args: &Args, _vectors: &[VectorItem], results: &mut BenchmarkResults) -> Result<()> {
    println!("📊 Index Operation Benchmarks");
    println!("-----------------------------");
    
    // Index creation
    let time = time_operation("Index Creation", args.iterations, || async {
        let temp_dir = tempfile::TempDir::new()?;
        let index = LocalIndex::new(temp_dir.path(), None)?;
        let config = CreateIndexConfig::default();
        index.create_index(Some(config)).await?;
        Ok(())
    }).await?;
    results.add("index_creation", time);
    
    // Index loading (properly isolated)
    let time = {
        // Pre-create an index with data (outside timing)
        let temp_dir = tempfile::TempDir::new()?;
        let index = LocalIndex::new(temp_dir.path(), None)?;
        let config = CreateIndexConfig::default();
        index.create_index(Some(config)).await?;
        
        // Add some data using bulk insert
        let mut test_data = TestDataGenerator::new(384);
        let vectors = test_data.generate_vectors(100);
        index.insert_items(vectors).await?;
        
        // Now measure just the loading part
        time_operation("Index Loading", args.iterations, || {
            let temp_dir_path = temp_dir.path().to_path_buf();
            async move {
                let _loaded = LocalIndex::new(&temp_dir_path, None)?;  
                Ok(())
            }
        }).await?
    };
    results.add("index_loading", time);
    
    Ok(())
}

async fn run_item_benchmarks(args: &Args, vectors: &[VectorItem], results: &mut BenchmarkResults) -> Result<()> {
    println!("📊 Item Operation Benchmarks");
    println!("----------------------------");
    
    // Single insert
    let test_vector = vectors[0].clone();
    let time = time_operation("Single Insert", args.iterations, || {
        let vector = test_vector.clone();
        async move {
            let temp_dir = tempfile::TempDir::new()?;
            let index = LocalIndex::new(temp_dir.path(), None)?;
            let config = CreateIndexConfig::default();
            index.create_index(Some(config)).await?;
            index.insert_item(vector).await?;
            Ok(())
        }
    }).await?;
    results.add("single_insert", time);
    
    // Batch insert - test both individual and bulk insert methods
    let batch_size = std::cmp::min(100, vectors.len());
    let batch = vectors[0..batch_size].to_vec();
    
    // Individual inserts (current method)
    let time = time_operation("Batch Insert (Individual)", args.iterations, || {
        let batch = batch.clone();
        async move {
            let temp_dir = tempfile::TempDir::new()?;
            let index = LocalIndex::new(temp_dir.path(), None)?;
            let config = CreateIndexConfig::default();
            index.create_index(Some(config)).await?;
            
            for item in batch {
                index.insert_item(item).await?;
            }
            Ok(())
        }
    }).await?;
    results.add("batch_insert_individual", time);
    
    // Bulk insert (optimized method)
    let time = time_operation("Batch Insert (Bulk)", args.iterations, || {
        let batch = batch.clone();
        async move {
            let temp_dir = tempfile::TempDir::new()?;
            let index = LocalIndex::new(temp_dir.path(), None)?;
            let config = CreateIndexConfig::default();
            index.create_index(Some(config)).await?;
            
            index.insert_items(batch).await?;
            Ok(())
        }
    }).await?;
    results.add("batch_insert_bulk", time);
    
    Ok(())
}

async fn run_search_benchmarks(args: &Args, vectors: &[VectorItem], query_vectors: &[VectorItem], results: &mut BenchmarkResults) -> Result<()> {
    println!("📊 Search Operation Benchmarks");
    println!("------------------------------");
    
    // Setup index with data
    let temp_dir = tempfile::TempDir::new()?;
    let index = LocalIndex::new(temp_dir.path(), None)?;
    let config = CreateIndexConfig::default();
    index.create_index(Some(config)).await?;
    
    // Use a reasonable dataset size for search benchmarks (100 items max)
    let data_size = std::cmp::min(100, vectors.len());
    for vector in &vectors[0..data_size] {
        index.insert_item(vector.clone()).await?;
    }
    
    // Single search
    let query = query_vectors[0].clone();
    let time = time_operation("Single Search", args.iterations, || {
        let query_vec = query.vector.clone();
        let index = &index;
        async move {
            let _results = index.query_items(query_vec, Some(10), None).await?;
            Ok(())
        }
    }).await?;
    results.add("single_search", time);
    
    Ok(())
}

async fn run_scale_benchmarks(_args: &Args, vectors: &[VectorItem], results: &mut BenchmarkResults) -> Result<()> {
    println!("📊 Scale Benchmarks");
    println!("------------------");
    
    // Large dataset creation
    let scale_sizes = [1000, 5000, 10000];
    
    for &size in &scale_sizes {
        if size <= vectors.len() {
            let subset = &vectors[0..size];
            let time = time_operation(&format!("Scale {} items", size), 1, || {
                let data = subset.to_vec();
                async move {
                    let temp_dir = tempfile::TempDir::new()?;
                    let index = LocalIndex::new(temp_dir.path(), None)?;
                    let config = CreateIndexConfig::default();
                    index.create_index(Some(config)).await?;
                    
                    // Use bulk insert for better performance
                    index.insert_items(data).await?;
                    Ok(())
                }
            }).await?;
            results.add(&format!("scale_{}_items", size), time);
        }
    }
    
    Ok(())
}

async fn time_operation<F, T>(name: &str, iterations: usize, op: F) -> Result<f64>
where
    F: Fn() -> T + Send + Sync,
    T: std::future::Future<Output = Result<()>> + Send,
{
    let pb = ProgressBar::new(iterations as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
    );
    pb.set_message(name.to_string());
    
    let mut total_time = 0.0;
    
    for _ in 0..iterations {
        let start = Instant::now();
        op().await?;
        let duration = start.elapsed();
        total_time += duration.as_secs_f64();
        pb.inc(1);
    }
    
    pb.finish_with_message(format!("{} - Average: {:.3}ms", name, (total_time / iterations as f64) * 1000.0));
    
    Ok(total_time / iterations as f64)
}

#[derive(Default)]
struct BenchmarkResults {
    results: std::collections::HashMap<String, f64>,
}

impl BenchmarkResults {
    fn new() -> Self {
        Self::default()
    }
    
    fn add(&mut self, name: &str, time: f64) {
        self.results.insert(name.to_string(), time);
    }
    
    async fn save(&self, output_dir: &PathBuf) -> Result<()> {
        tokio::fs::create_dir_all(output_dir).await?;
        
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = output_dir.join(format!("rust_benchmark_{}.json", timestamp));
        
        let json_data = json!({
            "timestamp": timestamp.to_string(),
            "implementation": "rust",
            "results": self.results
        });
        
        tokio::fs::write(filename, serde_json::to_string_pretty(&json_data)?).await?;
        Ok(())
    }
    
    fn print_summary(&self) {
        println!("📈 Benchmark Summary");
        println!("===================");
        
        let mut sorted_results: Vec<_> = self.results.iter().collect();
        sorted_results.sort_by(|a, b| a.0.cmp(b.0));
        
        for (name, time) in sorted_results {
            println!("{:30} {:>10.3}ms", name, time * 1000.0);
        }
    }
}