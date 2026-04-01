// Copyright 2024-2026 Andrey Vasilevsky <anvanster@gmail.com>
// SPDX-License-Identifier: Apache-2.0

//! Cypher query language parser for vectrust.
//!
//! Provides a [`logos`]-based lexer and hand-written recursive descent parser
//! that converts Cypher query strings into a strongly-typed AST.
//!
//! # Usage
//!
//! ```
//! let stmt = vectrust_cypher::parse("MATCH (n:Person) WHERE n.age > 25 RETURN n.name").unwrap();
//! assert_eq!(stmt.clauses.len(), 3);
//! ```

pub mod ast;
mod error;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use error::{CypherError, CypherResult};
pub use parser::Parser;

/// Parse a Cypher query string into an AST statement.
pub fn parse(input: &str) -> CypherResult<Statement> {
    Parser::new(input).parse()
}
