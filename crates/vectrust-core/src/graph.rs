use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A graph node with labels, properties, and optional vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: Uuid,
    pub labels: Vec<String>,
    pub properties: HashMap<String, GraphValue>,
}

/// A graph edge connecting two nodes with a type and properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: Uuid,
    pub source: Uuid,
    pub target: Uuid,
    pub rel_type: String,
    pub properties: HashMap<String, GraphValue>,
}

/// A path is a sequence of alternating nodes and edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// The universal value type returned by graph queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<GraphValue>),
    Map(HashMap<String, GraphValue>),
    Node(GraphNode),
    Edge(GraphEdge),
    Path(GraphPath),
}

impl GraphValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            GraphValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            GraphValue::Integer(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            GraphValue::Float(f) => Some(*f),
            GraphValue::Integer(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            GraphValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, GraphValue::Null)
    }

    pub fn as_node(&self) -> Option<&GraphNode> {
        match self {
            GraphValue::Node(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_edge(&self) -> Option<&GraphEdge> {
        match self {
            GraphValue::Edge(e) => Some(e),
            _ => None,
        }
    }
}

impl PartialEq for GraphValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (GraphValue::Null, GraphValue::Null) => true,
            (GraphValue::Bool(a), GraphValue::Bool(b)) => a == b,
            (GraphValue::Integer(a), GraphValue::Integer(b)) => a == b,
            (GraphValue::Float(a), GraphValue::Float(b)) => a == b,
            (GraphValue::String(a), GraphValue::String(b)) => a == b,
            (GraphValue::Integer(a), GraphValue::Float(b)) => (*a as f64) == *b,
            (GraphValue::Float(a), GraphValue::Integer(b)) => *a == (*b as f64),
            (GraphValue::List(a), GraphValue::List(b)) => a == b,
            (GraphValue::Map(a), GraphValue::Map(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialOrd for GraphValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (GraphValue::Integer(a), GraphValue::Integer(b)) => a.partial_cmp(b),
            (GraphValue::Float(a), GraphValue::Float(b)) => a.partial_cmp(b),
            (GraphValue::Integer(a), GraphValue::Float(b)) => (*a as f64).partial_cmp(b),
            (GraphValue::Float(a), GraphValue::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (GraphValue::String(a), GraphValue::String(b)) => a.partial_cmp(b),
            (GraphValue::Bool(a), GraphValue::Bool(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

impl From<serde_json::Value> for GraphValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => GraphValue::Null,
            serde_json::Value::Bool(b) => GraphValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    GraphValue::Integer(i)
                } else {
                    GraphValue::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            serde_json::Value::String(s) => GraphValue::String(s),
            serde_json::Value::Array(arr) => {
                GraphValue::List(arr.into_iter().map(GraphValue::from).collect())
            }
            serde_json::Value::Object(obj) => GraphValue::Map(
                obj.into_iter()
                    .map(|(k, v)| (k, GraphValue::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<GraphValue> for serde_json::Value {
    fn from(v: GraphValue) -> Self {
        match v {
            GraphValue::Null => serde_json::Value::Null,
            GraphValue::Bool(b) => serde_json::Value::Bool(b),
            GraphValue::Integer(n) => serde_json::json!(n),
            GraphValue::Float(f) => serde_json::json!(f),
            GraphValue::String(s) => serde_json::Value::String(s),
            GraphValue::List(arr) => {
                serde_json::Value::Array(arr.into_iter().map(serde_json::Value::from).collect())
            }
            GraphValue::Map(obj) => {
                let map: serde_json::Map<String, serde_json::Value> = obj
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect();
                serde_json::Value::Object(map)
            }
            GraphValue::Node(n) => serde_json::json!({
                "id": n.id.to_string(),
                "labels": n.labels,
                "properties": n.properties.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect::<serde_json::Map<String, serde_json::Value>>(),
            }),
            GraphValue::Edge(e) => serde_json::json!({
                "id": e.id.to_string(),
                "source": e.source.to_string(),
                "target": e.target.to_string(),
                "type": e.rel_type,
                "properties": e.properties.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect::<serde_json::Map<String, serde_json::Value>>(),
            }),
            GraphValue::Path(p) => serde_json::json!({
                "nodes": p.nodes.into_iter()
                    .map(|n| serde_json::Value::from(GraphValue::Node(n)))
                    .collect::<Vec<_>>(),
                "edges": p.edges.into_iter()
                    .map(|e| serde_json::Value::from(GraphValue::Edge(e)))
                    .collect::<Vec<_>>(),
            }),
        }
    }
}

/// Internal storage record for a node (lightweight, no properties).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    pub id: Uuid,
    pub labels: Vec<String>,
    pub has_vector: bool,
}

/// Internal storage record for an edge (lightweight, no properties).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRecord {
    pub id: Uuid,
    pub source: Uuid,
    pub target: Uuid,
    pub rel_type: String,
}

/// A single row in query results: maps column names to values.
pub type ResultRow = HashMap<String, GraphValue>;

/// Query result set.
#[derive(Debug, Clone)]
pub struct GraphQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<ResultRow>,
}

impl GraphQueryResult {
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }
}

/// Statistics about the graph database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub labels: Vec<String>,
    pub relationship_types: Vec<String>,
    pub has_vectors: bool,
}
