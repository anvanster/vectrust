// Copyright 2024-2026 Andrey Vasilevsky <anvanster@gmail.com>
// SPDX-License-Identifier: Apache-2.0

//! Real-world benchmark: index a real codebase with Jina Code V2 embeddings.
//!
//! Usage:
//!   cargo run --release --bin real_world_bench -- --project ~/projects/open-vm-tools
//!   cargo run --release --bin real_world_bench -- --project ~/projects/redamon

use anyhow::Result;
use clap::Parser;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "real_world_bench")]
struct Args {
    /// Path to the project to index
    #[arg(long)]
    project: PathBuf,

    /// Max files to index (0 = unlimited)
    #[arg(long, default_value = "0")]
    max_files: usize,

    /// Batch size for embedding
    #[arg(long, default_value = "32")]
    batch_size: usize,

    /// Cache directory for fastembed models
    #[arg(long, default_value = "~/.codegraph/fastembed_cache")]
    cache_dir: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let cache_dir = shellexpand::tilde(&args.cache_dir).to_string();

    println!("=== Real-World Vectrust Benchmark ===");
    println!("Project: {:?}", args.project);
    println!();

    // 1. Collect source files
    let start = Instant::now();
    let files = collect_source_files(&args.project, args.max_files);
    println!("1. Found {} source files in {:.1}ms", files.len(), start.elapsed().as_secs_f64() * 1000.0);

    if files.is_empty() {
        println!("No source files found!");
        return Ok(());
    }

    // 2. Read file contents
    let start = Instant::now();
    let mut documents: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        if let Ok(content) = std::fs::read_to_string(path) {
            // Truncate very long files for embedding
            let content = if content.len() > 8192 {
                content[..8192].to_string()
            } else {
                content
            };
            documents.push((path.clone(), content));
        }
    }
    println!("2. Read {} files in {:.1}ms", documents.len(), start.elapsed().as_secs_f64() * 1000.0);

    // 3. Initialize embedding model
    let start = Instant::now();
    let model = TextEmbedding::try_new(
        InitOptions::new(EmbeddingModel::JinaEmbeddingsV2BaseCode)
            .with_cache_dir(PathBuf::from(&cache_dir)),
    )?;
    println!("3. Loaded Jina Code V2 in {:.1}ms", start.elapsed().as_secs_f64() * 1000.0);

    // 4. Generate embeddings
    let start = Instant::now();
    let texts: Vec<&str> = documents.iter().map(|(_, c)| c.as_str()).collect();
    let mut all_embeddings: Vec<Vec<f32>> = Vec::new();
    for chunk in texts.chunks(args.batch_size) {
        let batch: Vec<String> = chunk.iter().map(|s| s.to_string()).collect();
        let embeddings = model.embed(batch, None)?;
        all_embeddings.extend(embeddings);
    }
    let embed_time = start.elapsed();
    println!(
        "4. Embedded {} files in {:.1}s ({:.1} files/sec, {}d vectors)",
        all_embeddings.len(),
        embed_time.as_secs_f64(),
        all_embeddings.len() as f64 / embed_time.as_secs_f64(),
        all_embeddings.first().map_or(0, |v| v.len()),
    );

    // 5. Create vectrust graph
    let tmp = tempfile::TempDir::new()?;
    let db = vectrust::GraphIndex::open(tmp.path())?;

    let start = Instant::now();
    for (i, ((path, _content), embedding)) in documents.iter().zip(&all_embeddings).enumerate() {
        let rel_path = path.strip_prefix(&args.project).unwrap_or(path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("unknown");
        let dir = rel_path.parent().and_then(|p| p.to_str()).unwrap_or("");

        db.create_node_with_vector(
            &["File"],
            serde_json::json!({
                "path": rel_path.to_string_lossy(),
                "name": path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                "ext": ext,
                "dir": dir,
                "index": i,
            }),
            embedding.clone(),
        )?;
    }
    let index_time = start.elapsed();
    println!(
        "5. Indexed {} nodes with vectors in {:.1}ms ({:.0} nodes/sec)",
        documents.len(),
        index_time.as_secs_f64() * 1000.0,
        documents.len() as f64 / index_time.as_secs_f64(),
    );

    // 6. Create edges (files in same directory -> SAME_DIR)
    let start = Instant::now();
    let result = db.cypher("MATCH (a:File), (b:File) WHERE a.dir = b.dir AND a.index < b.index RETURN count(*) AS potential")?;
    let potential = result.rows[0].get("potential").and_then(|v| v.as_i64()).unwrap_or(0);
    // Only create edges for files in same dir (capped to avoid explosion)
    db.cypher("MATCH (a:File), (b:File) WHERE a.dir = b.dir AND a.index < b.index AND a.index + 10 > b.index CREATE (a)-[:SAME_DIR]->(b)")?;
    let stats = db.graph_stats()?;
    println!(
        "6. Created {} edges in {:.1}ms (from {} potential same-dir pairs)",
        stats.edge_count,
        start.elapsed().as_secs_f64() * 1000.0,
        potential,
    );

    // 7. kNN search
    if let Some(query_vec) = all_embeddings.first() {
        let start = Instant::now();
        let result = db.cypher_with_params(
            "CALL vectrust.nearest('embedding', $q, 10) YIELD node, score RETURN node.path AS path, score",
            serde_json::json!({"q": query_vec}),
        )?;
        let knn_time = start.elapsed();
        println!(
            "\n7. kNN search (k=10, first call + HNSW build): {:.1}s",
            knn_time.as_secs_f64(),
        );
        println!("   Top results:");
        for row in result.rows.iter().take(5) {
            let path = row.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let score = row.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            println!("     {:.4}  {}", score, path);
        }

        // Cached search
        let start = Instant::now();
        let _result = db.cypher_with_params(
            "CALL vectrust.nearest('embedding', $q, 10) YIELD node, score RETURN node.path, score",
            serde_json::json!({"q": query_vec}),
        )?;
        println!("\n8. kNN cached: {:.2}ms", start.elapsed().as_secs_f64() * 1000.0);
    }

    // 8. Graph query
    let start = Instant::now();
    let result = db.cypher(
        "MATCH (f:File) RETURN f.ext AS ext, count(*) AS n ORDER BY n DESC LIMIT 10",
    )?;
    println!("\n9. Aggregation by extension ({:.1}ms):", start.elapsed().as_secs_f64() * 1000.0);
    for row in &result.rows {
        println!(
            "     .{}: {}",
            row.get("ext").and_then(|v| v.as_str()).unwrap_or("?"),
            row.get("n").and_then(|v| v.as_i64()).unwrap_or(0),
        );
    }

    // 9. Graph stats
    let stats = db.graph_stats()?;
    println!("\nFinal: {} nodes, {} edges, labels: {:?}", stats.node_count, stats.edge_count, stats.labels);
    println!("\n=== Benchmark Complete ===");

    Ok(())
}

fn collect_source_files(root: &Path, max: usize) -> Vec<PathBuf> {
    let source_exts = [
        "rs", "c", "h", "cpp", "cc", "hpp", "js", "ts", "tsx", "jsx",
        "py", "go", "java", "rb", "php", "swift", "kt", "cs", "lua",
        "sh", "bash", "zsh", "toml", "yaml", "yml", "json", "xml",
        "sql", "md", "txt", "cfg", "ini", "conf",
    ];

    let mut files = Vec::new();
    collect_recursive(root, &source_exts, &mut files, max);
    files
}

fn collect_recursive(dir: &Path, exts: &[&str], files: &mut Vec<PathBuf>, max: usize) {
    if max > 0 && files.len() >= max {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip hidden dirs, build dirs, node_modules
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == "build" {
                continue;
            }
            collect_recursive(&path, exts, files, max);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if exts.contains(&ext) {
                files.push(path);
                if max > 0 && files.len() >= max {
                    return;
                }
            }
        }
    }
}
