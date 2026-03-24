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
