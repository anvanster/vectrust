//! Graph storage and query execution for vectrust.
//!
//! This crate provides [`GraphStorage`] (RocksDB-backed graph with column families
//! for nodes, edges, adjacency lists, indexes, and vectors) and [`GraphExecutor`]
//! (Volcano-model query executor that processes Cypher AST against storage).
//!
//! Typically used through the `vectrust::GraphIndex` facade rather than directly.

pub mod executor;
pub mod storage;

pub use executor::GraphExecutor;
pub use storage::GraphStorage;
