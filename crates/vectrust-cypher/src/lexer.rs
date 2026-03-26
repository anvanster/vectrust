use logos::Logos;

/// Cypher token types produced by the logos-based lexer.
///
/// All keyword tokens are case-insensitive. Whitespace, `//` comments,
/// and `--` comments are skipped automatically.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"--[^\n]*")]
pub enum Token {
    // Keywords (case-insensitive)
    #[token("CREATE", ignore(ascii_case))]
    Create,
    #[token("MERGE", ignore(ascii_case))]
    Merge,
    #[token("ON", ignore(ascii_case))]
    On,
    #[token("MATCH", ignore(ascii_case))]
    Match,
    #[token("RETURN", ignore(ascii_case))]
    Return,
    #[token("WHERE", ignore(ascii_case))]
    Where,
    #[token("SET", ignore(ascii_case))]
    Set,
    #[token("DELETE", ignore(ascii_case))]
    Delete,
    #[token("DETACH", ignore(ascii_case))]
    Detach,
    #[token("REMOVE", ignore(ascii_case))]
    Remove,
    #[token("OPTIONAL", ignore(ascii_case))]
    Optional,
    #[token("ORDER", ignore(ascii_case))]
    Order,
    #[token("BY", ignore(ascii_case))]
    By,
    #[token("ASC", ignore(ascii_case))]
    Asc,
    #[token("DESC", ignore(ascii_case))]
    Desc,
    #[token("LIMIT", ignore(ascii_case))]
    Limit,
    #[token("SKIP", ignore(ascii_case))]
    Skip,
    #[token("WITH", ignore(ascii_case))]
    With,
    #[token("AS", ignore(ascii_case))]
    As,
    #[token("AND", ignore(ascii_case))]
    And,
    #[token("OR", ignore(ascii_case))]
    Or,
    #[token("NOT", ignore(ascii_case))]
    Not,
    #[token("IN", ignore(ascii_case))]
    In,
    #[token("CONTAINS", ignore(ascii_case))]
    Contains,
    #[token("STARTS", ignore(ascii_case))]
    Starts,
    #[token("ENDS", ignore(ascii_case))]
    Ends,
    #[token("TRUE", ignore(ascii_case))]
    True,
    #[token("FALSE", ignore(ascii_case))]
    False,
    #[token("NULL", ignore(ascii_case))]
    Null,
    #[token("IS", ignore(ascii_case))]
    Is,
    #[token("CALL", ignore(ascii_case))]
    Call,
    #[token("YIELD", ignore(ascii_case))]
    Yield,
    #[token("DISTINCT", ignore(ascii_case))]
    Distinct,
    #[token("COUNT", ignore(ascii_case))]
    Count,

    // Punctuation
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token(";")]
    Semicolon,
    #[token("|")]
    Pipe,

    // Arrow patterns
    #[token("->")]
    ArrowRight,
    #[token("<-")]
    ArrowLeft,
    #[token("-")]
    Dash,

    // Operators
    #[token("=")]
    Eq,
    #[token("<>")]
    Neq,
    #[token("!=")]
    Neq2,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    Lte,
    #[token(">=")]
    Gte,
    #[token("+")]
    Plus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,

    // Parameter
    #[regex(r"\$[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice()[1..].to_string())]
    Parameter(String),

    // Literals
    #[regex(r"-?[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    #[regex(r"-?[0-9]+", priority = 2, callback = |lex| lex.slice().parse::<i64>().ok())]
    Integer(i64),

    #[regex(r#"'([^'\\]|\\.)*'"#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].replace("\\'", "'").replace("\\\\", "\\"))
    })]
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].replace("\\\"", "\"").replace("\\\\", "\\"))
    })]
    StringLiteral(String),

    // Identifier (must come after keywords due to priority)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 1, callback = |lex| lex.slice().to_string())]
    Identifier(String),

    // Backtick-quoted identifier
    #[regex(r"`[^`]+`", |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    QuotedIdentifier(String),
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Create => write!(f, "CREATE"),
            Token::Merge => write!(f, "MERGE"),
            Token::On => write!(f, "ON"),
            Token::Match => write!(f, "MATCH"),
            Token::Return => write!(f, "RETURN"),
            Token::Where => write!(f, "WHERE"),
            Token::Set => write!(f, "SET"),
            Token::Delete => write!(f, "DELETE"),
            Token::Detach => write!(f, "DETACH"),
            Token::Remove => write!(f, "REMOVE"),
            Token::Optional => write!(f, "OPTIONAL"),
            Token::Order => write!(f, "ORDER"),
            Token::By => write!(f, "BY"),
            Token::Asc => write!(f, "ASC"),
            Token::Desc => write!(f, "DESC"),
            Token::Limit => write!(f, "LIMIT"),
            Token::Skip => write!(f, "SKIP"),
            Token::With => write!(f, "WITH"),
            Token::As => write!(f, "AS"),
            Token::And => write!(f, "AND"),
            Token::Or => write!(f, "OR"),
            Token::Not => write!(f, "NOT"),
            Token::In => write!(f, "IN"),
            Token::Contains => write!(f, "CONTAINS"),
            Token::Starts => write!(f, "STARTS"),
            Token::Ends => write!(f, "ENDS"),
            Token::True => write!(f, "TRUE"),
            Token::False => write!(f, "FALSE"),
            Token::Null => write!(f, "NULL"),
            Token::Is => write!(f, "IS"),
            Token::Call => write!(f, "CALL"),
            Token::Yield => write!(f, "YIELD"),
            Token::Distinct => write!(f, "DISTINCT"),
            Token::Count => write!(f, "COUNT"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Dot => write!(f, "."),
            Token::DotDot => write!(f, ".."),
            Token::Semicolon => write!(f, ";"),
            Token::Pipe => write!(f, "|"),
            Token::ArrowRight => write!(f, "->"),
            Token::ArrowLeft => write!(f, "<-"),
            Token::Dash => write!(f, "-"),
            Token::Eq => write!(f, "="),
            Token::Neq => write!(f, "<>"),
            Token::Neq2 => write!(f, "!="),
            Token::Lt => write!(f, "<"),
            Token::Gt => write!(f, ">"),
            Token::Lte => write!(f, "<="),
            Token::Gte => write!(f, ">="),
            Token::Plus => write!(f, "+"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::Parameter(name) => write!(f, "${name}"),
            Token::Float(v) => write!(f, "{v}"),
            Token::Integer(v) => write!(f, "{v}"),
            Token::StringLiteral(s) => write!(f, "'{s}'"),
            Token::Identifier(name) => write!(f, "{name}"),
            Token::QuotedIdentifier(name) => write!(f, "`{name}`"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<Token> {
        Token::lexer(input).filter_map(|r| r.ok()).collect()
    }

    #[test]
    fn test_create_node() {
        let tokens = lex("CREATE (n:Person {name: 'Alice', age: 30})");
        assert_eq!(
            tokens,
            vec![
                Token::Create,
                Token::LParen,
                Token::Identifier("n".into()),
                Token::Colon,
                Token::Identifier("Person".into()),
                Token::LBrace,
                Token::Identifier("name".into()),
                Token::Colon,
                Token::StringLiteral("Alice".into()),
                Token::Comma,
                Token::Identifier("age".into()),
                Token::Colon,
                Token::Integer(30),
                Token::RBrace,
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_match_return() {
        let tokens = lex("MATCH (n:Person) RETURN n.name");
        assert_eq!(
            tokens,
            vec![
                Token::Match,
                Token::LParen,
                Token::Identifier("n".into()),
                Token::Colon,
                Token::Identifier("Person".into()),
                Token::RParen,
                Token::Return,
                Token::Identifier("n".into()),
                Token::Dot,
                Token::Identifier("name".into()),
            ]
        );
    }

    #[test]
    fn test_edge_pattern() {
        let tokens = lex("MATCH (a)-[:KNOWS]->(b)");
        assert_eq!(
            tokens,
            vec![
                Token::Match,
                Token::LParen,
                Token::Identifier("a".into()),
                Token::RParen,
                Token::Dash,
                Token::LBracket,
                Token::Colon,
                Token::Identifier("KNOWS".into()),
                Token::RBracket,
                Token::ArrowRight,
                Token::LParen,
                Token::Identifier("b".into()),
                Token::RParen,
            ]
        );
    }

    #[test]
    fn test_parameter() {
        let tokens = lex("WHERE n.age > $min_age");
        assert_eq!(
            tokens,
            vec![
                Token::Where,
                Token::Identifier("n".into()),
                Token::Dot,
                Token::Identifier("age".into()),
                Token::Gt,
                Token::Parameter("min_age".into()),
            ]
        );
    }

    #[test]
    fn test_case_insensitive() {
        let tokens = lex("match (n) return n");
        assert_eq!(
            tokens,
            vec![
                Token::Match,
                Token::LParen,
                Token::Identifier("n".into()),
                Token::RParen,
                Token::Return,
                Token::Identifier("n".into()),
            ]
        );
    }

    #[test]
    fn test_float_literal() {
        let tokens = lex("0.8 3.14 -1.5");
        assert_eq!(
            tokens,
            vec![Token::Float(0.8), Token::Float(3.14), Token::Float(-1.5),]
        );
    }

    #[test]
    fn test_string_escapes() {
        let tokens = lex(r#"'it\'s' "say \"hello\"""#);
        assert_eq!(
            tokens,
            vec![
                Token::StringLiteral("it's".into()),
                Token::StringLiteral(r#"say "hello""#.into()),
            ]
        );
    }
}
