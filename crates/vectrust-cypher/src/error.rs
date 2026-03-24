use thiserror::Error;

pub type CypherResult<T> = std::result::Result<T, CypherError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum CypherError {
    #[error("Unexpected token at position {position}: expected {expected}, found {found}")]
    UnexpectedToken {
        position: usize,
        expected: String,
        found: String,
    },

    #[error("Unexpected end of input: expected {expected}")]
    UnexpectedEof { expected: String },

    #[error("Invalid syntax at position {position}: {message}")]
    InvalidSyntax { position: usize, message: String },

    #[error("Unsupported feature: {feature}")]
    Unsupported { feature: String },
}
