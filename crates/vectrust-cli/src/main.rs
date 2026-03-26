use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "vectrust")]
#[command(about = "Vectrust graph+vector database CLI")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Parser)]
enum Commands {
    /// Migrate index to optimized format
    Migrate {
        #[arg(short, long)]
        path: PathBuf,

        #[arg(short, long, default_value = "v2")]
        format: String,

        #[arg(long)]
        dry_run: bool,
    },

    /// Verify index integrity
    Verify {
        #[arg(short, long)]
        path: PathBuf,
    },

    /// Benchmark performance
    Bench {
        #[arg(short, long)]
        path: PathBuf,

        #[arg(long, default_value = "1000")]
        items: usize,
    },

    /// Show index statistics (vector storage)
    Stats {
        #[arg(short, long)]
        path: PathBuf,
    },

    /// Graph database commands
    Graph {
        #[command(subcommand)]
        command: GraphCommands,
    },
}

#[derive(Parser)]
enum GraphCommands {
    /// Show graph statistics (node/edge/label counts)
    Stats {
        #[arg(short, long)]
        path: PathBuf,
    },

    /// Execute a Cypher query
    Query {
        #[arg(short, long)]
        path: PathBuf,

        /// Cypher query string
        query: String,

        /// Parameters as JSON (e.g., '{"name": "Alice"}')
        #[arg(long)]
        params: Option<String>,
    },

    /// Create a node from the command line
    Create {
        #[arg(short, long)]
        path: PathBuf,

        /// Labels (comma-separated)
        #[arg(short, long)]
        labels: String,

        /// Properties as JSON
        #[arg(long, default_value = "{}")]
        props: String,
    },

    /// Export graph to JSON file
    Export {
        #[arg(short, long)]
        path: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Import graph from JSON file
    Import {
        #[arg(short, long)]
        path: PathBuf,

        /// JSON file to import
        #[arg(short, long)]
        file: PathBuf,
    },

    /// Rebuild all indexes from raw data
    Reindex {
        #[arg(short, long)]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Migrate {
            path,
            format,
            dry_run,
        } => {
            migrate_index(path, format, dry_run).await?;
        }
        Commands::Verify { path } => {
            verify_index(path).await?;
        }
        Commands::Bench { path, items } => {
            benchmark_index(path, items).await?;
        }
        Commands::Stats { path } => {
            show_vector_stats(path).await?;
        }
        Commands::Graph { command } => {
            handle_graph_command(command)?;
        }
    }

    Ok(())
}

fn handle_graph_command(command: GraphCommands) -> Result<()> {
    match command {
        GraphCommands::Stats { path } => graph_stats(path),
        GraphCommands::Query {
            path,
            query,
            params,
        } => graph_query(path, query, params),
        GraphCommands::Create {
            path,
            labels,
            props,
        } => graph_create(path, labels, props),
        GraphCommands::Export { path, output } => graph_export(path, output),
        GraphCommands::Import { path, file } => graph_import(path, file),
        GraphCommands::Reindex { path } => graph_reindex(path),
    }
}

fn graph_stats(path: PathBuf) -> Result<()> {
    let db = vectrust::GraphIndex::open(&path)?;
    let stats = db.graph_stats()?;

    println!("Graph Statistics for {:?}", path);
    println!("  Nodes: {}", stats.node_count);
    println!("  Edges: {}", stats.edge_count);
    println!(
        "  Vectors: {}",
        if stats.has_vectors { "yes" } else { "no" }
    );

    if !stats.labels.is_empty() {
        println!("  Labels: {}", stats.labels.join(", "));
    }
    if !stats.relationship_types.is_empty() {
        println!(
            "  Relationship types: {}",
            stats.relationship_types.join(", ")
        );
    }

    Ok(())
}

fn graph_query(path: PathBuf, query: String, params: Option<String>) -> Result<()> {
    let db = vectrust::GraphIndex::open(&path)?;

    let result = if let Some(params_str) = params {
        let params: serde_json::Value = serde_json::from_str(&params_str)?;
        db.cypher_with_params(&query, params)?
    } else {
        db.cypher(&query)?
    };

    if result.columns.is_empty() {
        println!(
            "Query executed successfully ({} rows affected)",
            result.rows.len()
        );
        return Ok(());
    }

    // Print header
    println!("{}", result.columns.join(" | "));
    println!(
        "{}",
        "-".repeat(result.columns.iter().map(|c| c.len() + 3).sum::<usize>())
    );

    // Print rows
    for row in &result.rows {
        let values: Vec<String> = result
            .columns
            .iter()
            .map(|col| {
                row.get(col)
                    .map(format_graph_value)
                    .unwrap_or_else(|| "null".to_string())
            })
            .collect();
        println!("{}", values.join(" | "));
    }

    println!("\n{} row(s)", result.rows.len());
    Ok(())
}

fn graph_create(path: PathBuf, labels: String, props: String) -> Result<()> {
    let db = vectrust::GraphIndex::open(&path)?;
    let label_list: Vec<&str> = labels.split(',').map(|s| s.trim()).collect();
    let properties: serde_json::Value = serde_json::from_str(&props)?;

    let node = db.create_node(&label_list, properties)?;
    println!("Created node {} with labels {:?}", node.id, node.labels);

    Ok(())
}

fn graph_export(path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let db = vectrust::GraphIndex::open(&path)?;
    let data = db.export_json()?;
    let json = serde_json::to_string_pretty(&data)?;

    if let Some(output_path) = output {
        std::fs::write(&output_path, &json)?;
        println!(
            "Exported {} nodes, {} edges to {:?}",
            data.nodes.len(),
            data.edges.len(),
            output_path
        );
    } else {
        println!("{}", json);
    }

    Ok(())
}

fn graph_import(path: PathBuf, file: PathBuf) -> Result<()> {
    let json = std::fs::read_to_string(&file)?;
    let data: vectrust::GraphJson = serde_json::from_str(&json)?;

    let db = vectrust::GraphIndex::open(&path)?;
    let (nodes, edges) = db.import_json(&data)?;

    println!("Imported {} nodes, {} edges from {:?}", nodes, edges, file);
    Ok(())
}

fn graph_reindex(path: PathBuf) -> Result<()> {
    let db = vectrust::GraphIndex::open(&path)?;
    let stats = db.graph_stats()?;

    println!("Reindexing {:?}...", path);
    println!("  Nodes: {}", stats.node_count);
    println!("  Edges: {}", stats.edge_count);

    // Rebuild property indexes for all labels
    for label in &stats.labels {
        // Get all property keys for this label by sampling nodes
        let nodes = db.nodes_by_label(label)?;
        let mut prop_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        for node in nodes.iter().take(100) {
            for key in node.properties.keys() {
                prop_keys.insert(key.clone());
            }
        }

        for key in &prop_keys {
            db.create_property_index(label, key)?;
            println!("  Indexed :{}({})", label, key);
        }
    }

    println!("Reindex complete.");
    Ok(())
}

fn format_graph_value(val: &vectrust::GraphValue) -> String {
    match val {
        vectrust::GraphValue::Null => "null".to_string(),
        vectrust::GraphValue::Bool(b) => b.to_string(),
        vectrust::GraphValue::Integer(n) => n.to_string(),
        vectrust::GraphValue::Float(f) => format!("{:.4}", f),
        vectrust::GraphValue::String(s) => format!("\"{}\"", s),
        vectrust::GraphValue::Node(n) => {
            format!("({}:{})", &n.id.to_string()[..8], n.labels.join(":"))
        }
        vectrust::GraphValue::Edge(e) => format!("[{}:{}]", &e.id.to_string()[..8], e.rel_type),
        vectrust::GraphValue::List(items) => {
            let inner: Vec<String> = items.iter().map(format_graph_value).collect();
            format!("[{}]", inner.join(", "))
        }
        vectrust::GraphValue::Map(m) => {
            let inner: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_graph_value(v)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        vectrust::GraphValue::Path(_) => "<path>".to_string(),
    }
}

async fn show_vector_stats(path: PathBuf) -> Result<()> {
    let index = vectrust::LocalIndex::new(&path, None)?;
    if !index.is_index_created().await {
        println!("No vector index found at {:?}", path);
        return Ok(());
    }
    let stats = index.get_stats().await?;
    println!("Vector Index Statistics for {:?}", path);
    println!("  Items: {}", stats.items);
    println!("  Dimensions: {:?}", stats.dimensions);
    println!("  Distance metric: {:?}", stats.distance_metric);
    Ok(())
}

async fn migrate_index(path: PathBuf, format: String, dry_run: bool) -> Result<()> {
    println!("Migrating index at {:?} to format {}", path, format);
    if dry_run {
        println!("DRY RUN - no changes will be made");
    }
    // TODO: Implement migration logic
    Ok(())
}

async fn verify_index(path: PathBuf) -> Result<()> {
    println!("Verifying index at {:?}", path);
    // TODO: Implement verification logic
    Ok(())
}

async fn benchmark_index(path: PathBuf, items: usize) -> Result<()> {
    println!("Benchmarking index at {:?} with {} items", path, items);
    // TODO: Implement benchmark logic
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrate_function() {
        let path = PathBuf::from("/tmp/test");
        let result = migrate_index(path, "v2".to_string(), true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_function() {
        let path = PathBuf::from("/tmp/test");
        let result = verify_index(path).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;

        let args = vec!["vectrust", "stats", "--path", "/tmp/test"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_graph_cli_parsing() {
        use clap::Parser;

        let args = vec![
            "vectrust",
            "graph",
            "query",
            "--path",
            "/tmp/test",
            "MATCH (n) RETURN n",
        ];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_graph_stats_cli_parsing() {
        use clap::Parser;

        let args = vec!["vectrust", "graph", "stats", "--path", "/tmp/test"];
        let cli = Cli::try_parse_from(args);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_graph_query_execution() {
        let dir = tempfile::TempDir::new().unwrap();
        let db = vectrust::GraphIndex::open(dir.path()).unwrap();
        db.cypher("CREATE (n:Person {name: 'Alice'})").unwrap();

        let result = db.cypher("MATCH (n:Person) RETURN n.name AS name").unwrap();
        assert_eq!(result.rows.len(), 1);
        assert_eq!(
            result.rows[0].get("name"),
            Some(&vectrust::GraphValue::String("Alice".into()))
        );
    }
}
