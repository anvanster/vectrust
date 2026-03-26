use crate::ast::*;
use crate::error::{CypherError, CypherResult};
use crate::lexer::Token;
use logos::Logos;

/// Spanned token: (token, byte offset in source)
#[derive(Debug, Clone)]
struct Spanned {
    token: Token,
    span_start: usize,
}

/// Recursive descent parser for a Cypher subset.
///
/// Supports: CREATE, MATCH, OPTIONAL MATCH, MERGE, WHERE, RETURN, SET,
/// DELETE, DETACH DELETE, REMOVE, ORDER BY, LIMIT, SKIP, WITH, CALL...YIELD.
pub struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    /// Create a new parser from a Cypher query string.
    pub fn new(input: &str) -> Self {
        let lexer = Token::lexer(input);
        let tokens: Vec<Spanned> = lexer
            .spanned()
            .filter_map(|(result, span)| {
                result.ok().map(|token| Spanned {
                    token,
                    span_start: span.start,
                })
            })
            .collect();
        Self { tokens, pos: 0 }
    }

    /// Parse the full input into a Statement.
    pub fn parse(&mut self) -> CypherResult<Statement> {
        let mut clauses = Vec::new();

        while !self.at_end() {
            // Skip optional semicolons between statements
            if self.check(&Token::Semicolon) {
                self.advance();
                continue;
            }
            let clause = self.parse_clause()?;
            clauses.push(clause);
        }

        if clauses.is_empty() {
            return Err(CypherError::InvalidSyntax {
                position: 0,
                message: "empty query".to_string(),
            });
        }

        Ok(Statement { clauses })
    }

    // ─── Clause parsing ──────────────────────────────────────────

    fn parse_clause(&mut self) -> CypherResult<Clause> {
        match self.peek_token() {
            Some(Token::Match) => self.parse_match_clause(false),
            Some(Token::Optional) => {
                self.advance();
                self.parse_match_clause(true)
            }
            Some(Token::Create) => self.parse_create(),
            Some(Token::Merge) => self.parse_merge(),
            Some(Token::Return) => self.parse_return(),
            Some(Token::Where) => self.parse_where(),
            Some(Token::Set) => self.parse_set(),
            Some(Token::Delete) => self.parse_delete(),
            Some(Token::Detach) => self.parse_detach_delete(),
            Some(Token::Remove) => self.parse_remove(),
            Some(Token::Order) => self.parse_order_by(),
            Some(Token::Limit) => self.parse_limit(),
            Some(Token::Skip) => self.parse_skip(),
            Some(Token::With) => self.parse_with(),
            Some(Token::Unwind) => self.parse_unwind(),
            Some(Token::Call) => self.parse_call(),
            Some(other) => Err(CypherError::UnexpectedToken {
                position: self.current_position(),
                expected: "clause keyword (MATCH, CREATE, RETURN, WHERE, ...)".to_string(),
                found: other.to_string(),
            }),
            None => Err(CypherError::UnexpectedEof {
                expected: "clause keyword".to_string(),
            }),
        }
    }

    fn parse_match_clause(&mut self, optional: bool) -> CypherResult<Clause> {
        self.expect_token(&Token::Match)?;
        let patterns = self.parse_pattern_list()?;
        Ok(Clause::Match(MatchClause { patterns, optional }))
    }

    fn parse_create(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Create)?;
        let patterns = self.parse_pattern_list()?;
        Ok(Clause::Create(CreateClause { patterns }))
    }

    fn parse_merge(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Merge)?;
        let pattern = self.parse_pattern()?;

        let mut on_create = Vec::new();
        let mut on_match = Vec::new();

        // Parse optional ON CREATE SET / ON MATCH SET
        while self.eat_if(&Token::On) {
            match self.peek_token() {
                Some(Token::Create) => {
                    self.advance();
                    self.expect_token(&Token::Set)?;
                    loop {
                        let target = self.parse_postfix()?;
                        self.expect_token(&Token::Eq)?;
                        let value = self.parse_expression()?;
                        on_create.push(SetItem::Property { target, value });
                        if !self.eat_if(&Token::Comma) {
                            break;
                        }
                    }
                }
                Some(Token::Match) => {
                    self.advance();
                    self.expect_token(&Token::Set)?;
                    loop {
                        let target = self.parse_postfix()?;
                        self.expect_token(&Token::Eq)?;
                        let value = self.parse_expression()?;
                        on_match.push(SetItem::Property { target, value });
                        if !self.eat_if(&Token::Comma) {
                            break;
                        }
                    }
                }
                Some(other) => {
                    return Err(CypherError::UnexpectedToken {
                        position: self.current_position(),
                        expected: "CREATE or MATCH after ON".into(),
                        found: other.to_string(),
                    });
                }
                None => {
                    return Err(CypherError::UnexpectedEof {
                        expected: "CREATE or MATCH after ON".into(),
                    });
                }
            }
        }

        Ok(Clause::Merge(MergeClause {
            pattern,
            on_create,
            on_match,
        }))
    }

    fn parse_return(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Return)?;
        let distinct = self.eat_if(&Token::Distinct);
        let items = self.parse_return_items()?;
        Ok(Clause::Return(ReturnClause { distinct, items }))
    }

    fn parse_where(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Where)?;
        let expression = self.parse_expression()?;
        Ok(Clause::Where(WhereClause { expression }))
    }

    fn parse_set(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Set)?;
        let mut items = Vec::new();
        loop {
            // Target is a property access (not a full expression, to avoid consuming `=`)
            let target = self.parse_postfix()?;
            self.expect_token(&Token::Eq)?;
            let value = self.parse_expression()?;
            items.push(SetItem::Property { target, value });
            if !self.eat_if(&Token::Comma) {
                break;
            }
        }
        Ok(Clause::Set(SetClause { items }))
    }

    fn parse_delete(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Delete)?;
        let expressions = self.parse_expression_list()?;
        Ok(Clause::Delete(DeleteClause {
            detach: false,
            expressions,
        }))
    }

    fn parse_detach_delete(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Detach)?;
        self.expect_token(&Token::Delete)?;
        let expressions = self.parse_expression_list()?;
        Ok(Clause::Delete(DeleteClause {
            detach: true,
            expressions,
        }))
    }

    fn parse_remove(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Remove)?;
        let items = self.parse_expression_list()?;
        Ok(Clause::Remove(RemoveClause { items }))
    }

    fn parse_order_by(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Order)?;
        self.expect_token(&Token::By)?;
        let mut items = Vec::new();
        loop {
            let expression = self.parse_expression()?;
            let descending = if self.eat_if(&Token::Desc) {
                true
            } else {
                self.eat_if(&Token::Asc);
                false
            };
            items.push(OrderItem {
                expression,
                descending,
            });
            if !self.eat_if(&Token::Comma) {
                break;
            }
        }
        Ok(Clause::OrderBy(OrderByClause { items }))
    }

    fn parse_limit(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Limit)?;
        let count = self.parse_primary()?;
        Ok(Clause::Limit(LimitClause { count }))
    }

    fn parse_skip(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Skip)?;
        let count = self.parse_primary()?;
        Ok(Clause::Skip(SkipClause { count }))
    }

    fn parse_with(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::With)?;
        let distinct = self.eat_if(&Token::Distinct);
        let items = self.parse_return_items()?;
        Ok(Clause::With(WithClause { distinct, items }))
    }

    fn parse_unwind(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Unwind)?;
        let expression = self.parse_expression()?;
        self.expect_token(&Token::As)?;
        let alias = self.expect_identifier()?;
        Ok(Clause::Unwind(UnwindClause { expression, alias }))
    }

    fn parse_call(&mut self) -> CypherResult<Clause> {
        self.expect_token(&Token::Call)?;

        // Parse procedure name: identifier.identifier.identifier...
        let mut parts = vec![self.expect_identifier()?];
        while self.eat_if(&Token::Dot) {
            parts.push(self.expect_identifier()?);
        }
        let procedure = parts.join(".");

        // Parse arguments
        self.expect_token(&Token::LParen)?;
        let mut args = Vec::new();
        if !self.check(&Token::RParen) {
            args.push(self.parse_expression()?);
            while self.eat_if(&Token::Comma) {
                args.push(self.parse_expression()?);
            }
        }
        self.expect_token(&Token::RParen)?;

        // Parse YIELD
        let mut yields = Vec::new();
        if self.eat_if(&Token::Yield) {
            loop {
                let name = self.expect_identifier()?;
                let alias = if self.eat_if(&Token::As) {
                    Some(self.expect_identifier()?)
                } else {
                    None
                };
                yields.push(YieldItem { name, alias });
                if !self.eat_if(&Token::Comma) {
                    break;
                }
            }
        }

        Ok(Clause::Call(CallClause {
            procedure,
            args,
            yields,
        }))
    }

    // ─── Pattern parsing ─────────────────────────────────────────

    fn parse_pattern_list(&mut self) -> CypherResult<Vec<Pattern>> {
        let mut patterns = Vec::new();
        patterns.push(self.parse_pattern()?);
        while self.eat_if(&Token::Comma) {
            patterns.push(self.parse_pattern()?);
        }
        Ok(patterns)
    }

    /// Parse a pattern: (node)-[rel]->(node)-[rel]->(node)...
    fn parse_pattern(&mut self) -> CypherResult<Pattern> {
        let mut elements = Vec::new();

        // Must start with a node
        elements.push(PatternElement::Node(self.parse_node_pattern()?));

        // Then alternating relationship-node pairs
        while self.check(&Token::Dash)
            || self.check(&Token::ArrowLeft)
            || self.check(&Token::ArrowRight)
        {
            let rel = self.parse_relationship_pattern()?;
            elements.push(PatternElement::Relationship(rel));
            elements.push(PatternElement::Node(self.parse_node_pattern()?));
        }

        Ok(Pattern { elements })
    }

    fn parse_node_pattern(&mut self) -> CypherResult<NodePattern> {
        self.expect_token(&Token::LParen)?;

        let variable = self.try_parse_identifier();
        let mut labels = Vec::new();
        while self.eat_if(&Token::Colon) {
            labels.push(self.expect_identifier()?);
        }
        let properties = if self.check(&Token::LBrace) {
            Some(self.parse_map_literal()?)
        } else {
            None
        };

        self.expect_token(&Token::RParen)?;

        Ok(NodePattern {
            variable,
            labels,
            properties,
        })
    }

    fn parse_relationship_pattern(&mut self) -> CypherResult<RelationshipPattern> {
        // Patterns:  -[...]->  |  <-[...]-  |  -[...]-  |  -->  |  <--  |  --
        let left_arrow = self.eat_if(&Token::ArrowLeft);
        if !left_arrow {
            // Could be  -[...]->  or  -[...]-  or  -->  or  --
            // Check for -> (no brackets, directed right)
            if self.eat_if(&Token::ArrowRight) {
                return Ok(RelationshipPattern {
                    variable: None,
                    rel_types: vec![],
                    properties: None,
                    direction: Direction::OutRight,
                    length: None,
                });
            }
            self.expect_token(&Token::Dash)?;
        }

        // Optional bracket details
        let (variable, rel_types, properties, length) = if self.eat_if(&Token::LBracket) {
            let var = self.try_parse_identifier();
            let mut types = Vec::new();
            while self.eat_if(&Token::Colon) {
                types.push(self.expect_identifier()?);
            }
            let len = self.try_parse_variable_length()?;
            let props = if self.check(&Token::LBrace) {
                Some(self.parse_map_literal()?)
            } else {
                None
            };
            self.expect_token(&Token::RBracket)?;
            (var, types, props, len)
        } else {
            (None, vec![], None, None)
        };

        // Right side: -> or -
        let right_arrow = self.eat_if(&Token::ArrowRight);
        if !right_arrow {
            // Both <-[...]- and -[...]- have a trailing dash to consume
            self.expect_token(&Token::Dash)?;
        }

        let direction = match (left_arrow, right_arrow) {
            (false, true) => Direction::OutRight,
            (true, false) => Direction::InLeft,
            (false, false) => Direction::Both,
            (true, true) => {
                return Err(CypherError::InvalidSyntax {
                    position: self.current_position(),
                    message: "bidirectional arrow <-[...]-> is not valid".to_string(),
                });
            }
        };

        Ok(RelationshipPattern {
            variable,
            rel_types,
            properties,
            direction,
            length,
        })
    }

    fn try_parse_variable_length(&mut self) -> CypherResult<Option<(Option<u32>, Option<u32>)>> {
        if !self.eat_if(&Token::Star) {
            return Ok(None);
        }
        // *  → (None, None) = any length
        // *2 → (Some(2), Some(2)) = exactly 2
        // *1..3 → (Some(1), Some(3))
        // *..3 → (None, Some(3))
        // *1.. → (Some(1), None)
        let min = self.try_parse_u32();
        if self.eat_if(&Token::DotDot) {
            let max = self.try_parse_u32();
            Ok(Some((min, max)))
        } else if let Some(n) = min {
            Ok(Some((Some(n), Some(n))))
        } else {
            Ok(Some((None, None)))
        }
    }

    fn try_parse_u32(&mut self) -> Option<u32> {
        if let Some(Token::Integer(n)) = self.peek_token() {
            if n >= 0 {
                self.advance();
                return Some(n as u32);
            }
        }
        None
    }

    // ─── Expression parsing (precedence climbing) ────────────────

    fn parse_expression(&mut self) -> CypherResult<Expression> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> CypherResult<Expression> {
        let mut left = self.parse_and()?;
        while self.eat_if(&Token::Or) {
            let right = self.parse_and()?;
            left = Expression::BoolOp {
                left: Box::new(left),
                op: BooleanOp::Or,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> CypherResult<Expression> {
        let mut left = self.parse_not()?;
        while self.eat_if(&Token::And) {
            let right = self.parse_not()?;
            left = Expression::BoolOp {
                left: Box::new(left),
                op: BooleanOp::And,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> CypherResult<Expression> {
        if self.eat_if(&Token::Not) {
            let expr = self.parse_not()?;
            return Ok(Expression::Not(Box::new(expr)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> CypherResult<Expression> {
        let left = self.parse_addition()?;

        // IS NULL / IS NOT NULL
        if self.eat_if(&Token::Is) {
            let negated = self.eat_if(&Token::Not);
            self.expect_token(&Token::Null)?;
            return Ok(Expression::IsNull {
                expression: Box::new(left),
                negated,
            });
        }

        // String/collection operators
        if self.eat_if(&Token::Contains) {
            let right = self.parse_addition()?;
            return Ok(Expression::StringOp {
                left: Box::new(left),
                op: StringMatchOp::Contains,
                right: Box::new(right),
            });
        }
        if self.eat_if(&Token::In) {
            let right = self.parse_addition()?;
            return Ok(Expression::StringOp {
                left: Box::new(left),
                op: StringMatchOp::In,
                right: Box::new(right),
            });
        }
        if self.eat_if(&Token::Starts) {
            // STARTS WITH
            self.expect_identifier_value("WITH")?;
            let right = self.parse_addition()?;
            return Ok(Expression::StringOp {
                left: Box::new(left),
                op: StringMatchOp::StartsWith,
                right: Box::new(right),
            });
        }
        if self.eat_if(&Token::Ends) {
            // ENDS WITH
            self.expect_identifier_value("WITH")?;
            let right = self.parse_addition()?;
            return Ok(Expression::StringOp {
                left: Box::new(left),
                op: StringMatchOp::EndsWith,
                right: Box::new(right),
            });
        }

        // Comparison operators
        let op = match self.peek_token() {
            Some(Token::Eq) => Some(ComparisonOp::Eq),
            Some(Token::Neq | Token::Neq2) => Some(ComparisonOp::Neq),
            Some(Token::Lt) => Some(ComparisonOp::Lt),
            Some(Token::Gt) => Some(ComparisonOp::Gt),
            Some(Token::Lte) => Some(ComparisonOp::Lte),
            Some(Token::Gte) => Some(ComparisonOp::Gte),
            _ => None,
        };

        if let Some(op) = op {
            self.advance();
            let right = self.parse_addition()?;
            return Ok(Expression::Comparison {
                left: Box::new(left),
                op,
                right: Box::new(right),
            });
        }

        Ok(left)
    }

    fn parse_addition(&mut self) -> CypherResult<Expression> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.peek_token() {
                Some(Token::Plus) => ArithmeticOp::Add,
                Some(Token::Dash) => {
                    // Disambiguate: dash could be subtraction or start of relationship pattern
                    // Only treat as subtraction if we're NOT at the start of a pattern
                    if self
                        .peek_ahead(1)
                        .is_some_and(|t| matches!(t, Token::LBracket | Token::LParen))
                    {
                        break;
                    }
                    ArithmeticOp::Subtract
                }
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expression::Arithmetic {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> CypherResult<Expression> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_token() {
                Some(Token::Star) => ArithmeticOp::Multiply,
                Some(Token::Slash) => ArithmeticOp::Divide,
                Some(Token::Percent) => ArithmeticOp::Modulo,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expression::Arithmetic {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> CypherResult<Expression> {
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> CypherResult<Expression> {
        let mut expr = self.parse_primary()?;

        // Property access chain: n.prop.subprop
        while self.eat_if(&Token::Dot) {
            let key = self.expect_identifier()?;
            expr = Expression::Property {
                object: Box::new(expr),
                key,
            };
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> CypherResult<Expression> {
        match self.peek_token() {
            Some(Token::Integer(n)) => {
                self.advance();
                Ok(Expression::Literal(Literal::Integer(n)))
            }
            Some(Token::Float(f)) => {
                self.advance();
                Ok(Expression::Literal(Literal::Float(f)))
            }
            Some(Token::StringLiteral(s)) => {
                self.advance();
                Ok(Expression::Literal(Literal::String(s)))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expression::Literal(Literal::Bool(true)))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expression::Literal(Literal::Bool(false)))
            }
            Some(Token::Null) => {
                self.advance();
                Ok(Expression::Literal(Literal::Null))
            }
            Some(Token::Parameter(name)) => {
                self.advance();
                Ok(Expression::Parameter(name))
            }
            Some(Token::Star) => {
                self.advance();
                Ok(Expression::Star)
            }
            Some(Token::LBracket) => {
                self.advance();
                let mut elements = Vec::new();
                if !self.check(&Token::RBracket) {
                    elements.push(self.parse_expression()?);
                    while self.eat_if(&Token::Comma) {
                        elements.push(self.parse_expression()?);
                    }
                }
                self.expect_token(&Token::RBracket)?;
                Ok(Expression::ListLiteral(elements))
            }
            Some(Token::LBrace) => {
                let map = self.parse_map_literal()?;
                Ok(Expression::MapLiteral(map))
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect_token(&Token::RParen)?;
                Ok(expr)
            }
            // count(...) is special because COUNT is a keyword
            // But "count" can also be used as a variable/alias (e.g., ORDER BY count)
            Some(Token::Count) => {
                self.advance();
                if self.check(&Token::LParen) {
                    self.advance(); // consume (
                    let distinct = self.eat_if(&Token::Distinct);
                    let mut args = vec![self.parse_expression()?];
                    while self.eat_if(&Token::Comma) {
                        args.push(self.parse_expression()?);
                    }
                    self.expect_token(&Token::RParen)?;
                    Ok(Expression::FunctionCall {
                        name: "count".to_string(),
                        args,
                        distinct,
                    })
                } else {
                    // Used as a variable reference (e.g., alias "count")
                    Ok(Expression::Variable("count".to_string()))
                }
            }
            Some(Token::Identifier(name)) => {
                self.advance();
                // Check if this is a function call
                if self.check(&Token::LParen) {
                    self.advance();
                    let distinct = self.eat_if(&Token::Distinct);
                    let mut args = Vec::new();
                    if !self.check(&Token::RParen) {
                        args.push(self.parse_expression()?);
                        while self.eat_if(&Token::Comma) {
                            args.push(self.parse_expression()?);
                        }
                    }
                    self.expect_token(&Token::RParen)?;
                    Ok(Expression::FunctionCall {
                        name,
                        args,
                        distinct,
                    })
                } else {
                    Ok(Expression::Variable(name))
                }
            }
            Some(Token::QuotedIdentifier(name)) => {
                self.advance();
                Ok(Expression::Variable(name))
            }
            Some(Token::Case) => self.parse_case_when(),
            Some(other) => Err(CypherError::UnexpectedToken {
                position: self.current_position(),
                expected: "expression".to_string(),
                found: other.to_string(),
            }),
            None => Err(CypherError::UnexpectedEof {
                expected: "expression".to_string(),
            }),
        }
    }

    /// Parse CASE WHEN expr THEN expr [WHEN ...] [ELSE expr] END
    fn parse_case_when(&mut self) -> CypherResult<Expression> {
        self.expect_token(&Token::Case)?;

        // Check for simple CASE (CASE expr WHEN val THEN ...)
        let operand = if !self.check(&Token::When) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        let mut when_clauses = Vec::new();
        while self.eat_if(&Token::When) {
            let condition = self.parse_expression()?;
            self.expect_token(&Token::Then)?;
            let result = self.parse_expression()?;
            when_clauses.push((condition, result));
        }

        let else_clause = if self.eat_if(&Token::Else) {
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        self.expect_token(&Token::End)?;

        Ok(Expression::CaseWhen {
            operand,
            when_clauses,
            else_clause,
        })
    }

    fn parse_map_literal(&mut self) -> CypherResult<MapLiteral> {
        self.expect_token(&Token::LBrace)?;
        let mut entries = Vec::new();
        if !self.check(&Token::RBrace) {
            loop {
                let key = self.expect_identifier()?;
                self.expect_token(&Token::Colon)?;
                let value = self.parse_expression()?;
                entries.push((key, value));
                if !self.eat_if(&Token::Comma) {
                    break;
                }
            }
        }
        self.expect_token(&Token::RBrace)?;
        Ok(MapLiteral { entries })
    }

    fn parse_return_items(&mut self) -> CypherResult<Vec<ReturnItem>> {
        let mut items = Vec::new();
        loop {
            let expression = self.parse_expression()?;
            let alias = if self.eat_if(&Token::As) {
                Some(self.expect_identifier()?)
            } else {
                None
            };
            items.push(ReturnItem { expression, alias });
            if !self.eat_if(&Token::Comma) {
                break;
            }
        }
        Ok(items)
    }

    fn parse_expression_list(&mut self) -> CypherResult<Vec<Expression>> {
        let mut exprs = Vec::new();
        exprs.push(self.parse_expression()?);
        while self.eat_if(&Token::Comma) {
            exprs.push(self.parse_expression()?);
        }
        Ok(exprs)
    }

    // ─── Token utilities ─────────────────────────────────────────

    fn peek_token(&self) -> Option<Token> {
        self.tokens.get(self.pos).map(|s| s.token.clone())
    }

    fn peek_ahead(&self, offset: usize) -> Option<Token> {
        self.tokens.get(self.pos + offset).map(|s| s.token.clone())
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].token.clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn check(&self, expected: &Token) -> bool {
        self.peek_token()
            .is_some_and(|t| std::mem::discriminant(&t) == std::mem::discriminant(expected))
    }

    fn eat_if(&mut self, expected: &Token) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_token(&mut self, expected: &Token) -> CypherResult<Token> {
        if self.check(expected) {
            Ok(self.advance().unwrap())
        } else {
            match self.peek_token() {
                Some(found) => Err(CypherError::UnexpectedToken {
                    position: self.current_position(),
                    expected: expected.to_string(),
                    found: found.to_string(),
                }),
                None => Err(CypherError::UnexpectedEof {
                    expected: expected.to_string(),
                }),
            }
        }
    }

    fn expect_identifier(&mut self) -> CypherResult<String> {
        match self.peek_token() {
            Some(Token::Identifier(name)) => {
                self.advance();
                Ok(name)
            }
            Some(Token::QuotedIdentifier(name)) => {
                self.advance();
                Ok(name)
            }
            // Allow some keywords to be used as identifiers in property/label position
            Some(Token::Count | Token::Asc | Token::Desc | Token::Call | Token::Yield) => {
                let tok = self.advance().unwrap();
                Ok(tok.to_string())
            }
            Some(found) => Err(CypherError::UnexpectedToken {
                position: self.current_position(),
                expected: "identifier".to_string(),
                found: found.to_string(),
            }),
            None => Err(CypherError::UnexpectedEof {
                expected: "identifier".to_string(),
            }),
        }
    }

    fn expect_identifier_value(&mut self, value: &str) -> CypherResult<()> {
        match self.peek_token() {
            Some(Token::Identifier(ref name)) if name.eq_ignore_ascii_case(value) => {
                self.advance();
                Ok(())
            }
            Some(Token::With) if value.eq_ignore_ascii_case("WITH") => {
                self.advance();
                Ok(())
            }
            Some(found) => Err(CypherError::UnexpectedToken {
                position: self.current_position(),
                expected: value.to_string(),
                found: found.to_string(),
            }),
            None => Err(CypherError::UnexpectedEof {
                expected: value.to_string(),
            }),
        }
    }

    fn try_parse_identifier(&mut self) -> Option<String> {
        match self.peek_token() {
            Some(Token::Identifier(name)) => {
                self.advance();
                Some(name)
            }
            Some(Token::QuotedIdentifier(name)) => {
                self.advance();
                Some(name)
            }
            _ => None,
        }
    }

    fn current_position(&self) -> usize {
        self.tokens.get(self.pos).map_or(0, |s| s.span_start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_create_node() {
        let stmt = parse("CREATE (n:Person {name: 'Alice', age: 30})").unwrap();
        assert_eq!(stmt.clauses.len(), 1);
        match &stmt.clauses[0] {
            Clause::Create(c) => {
                assert_eq!(c.patterns.len(), 1);
                match &c.patterns[0].elements[0] {
                    PatternElement::Node(n) => {
                        assert_eq!(n.variable, Some("n".into()));
                        assert_eq!(n.labels, vec!["Person".to_string()]);
                        let props = n.properties.as_ref().unwrap();
                        assert_eq!(props.entries.len(), 2);
                        assert_eq!(props.entries[0].0, "name");
                        assert_eq!(props.entries[1].0, "age");
                    }
                    _ => panic!("expected node pattern"),
                }
            }
            _ => panic!("expected CREATE clause"),
        }
    }

    #[test]
    fn test_match_return() {
        let stmt = parse("MATCH (n:Person) RETURN n.name, n.age").unwrap();
        assert_eq!(stmt.clauses.len(), 2);
        assert!(matches!(&stmt.clauses[0], Clause::Match(_)));
        match &stmt.clauses[1] {
            Clause::Return(r) => {
                assert_eq!(r.items.len(), 2);
                assert!(!r.distinct);
            }
            _ => panic!("expected RETURN clause"),
        }
    }

    #[test]
    fn test_match_edge_pattern() {
        let stmt = parse("MATCH (a)-[:KNOWS]->(b) RETURN b").unwrap();
        assert_eq!(stmt.clauses.len(), 2);
        match &stmt.clauses[0] {
            Clause::Match(m) => {
                let pattern = &m.patterns[0];
                assert_eq!(pattern.elements.len(), 3);
                match &pattern.elements[1] {
                    PatternElement::Relationship(r) => {
                        assert_eq!(r.rel_types, vec!["KNOWS".to_string()]);
                        assert_eq!(r.direction, Direction::OutRight);
                    }
                    _ => panic!("expected relationship"),
                }
            }
            _ => panic!("expected MATCH clause"),
        }
    }

    #[test]
    fn test_where_comparison() {
        let stmt = parse("MATCH (n:Person) WHERE n.age > 25 RETURN n").unwrap();
        assert_eq!(stmt.clauses.len(), 3);
        match &stmt.clauses[1] {
            Clause::Where(w) => {
                assert!(matches!(
                    &w.expression,
                    Expression::Comparison {
                        op: ComparisonOp::Gt,
                        ..
                    }
                ));
            }
            _ => panic!("expected WHERE clause"),
        }
    }

    #[test]
    fn test_where_and_or() {
        let stmt = parse("MATCH (n) WHERE n.a = 1 AND n.b = 2 OR n.c = 3 RETURN n").unwrap();
        assert_eq!(stmt.clauses.len(), 3);
        match &stmt.clauses[1] {
            Clause::Where(w) => {
                // Should be OR at top (lower precedence), with AND on the left
                assert!(matches!(
                    &w.expression,
                    Expression::BoolOp {
                        op: BooleanOp::Or,
                        ..
                    }
                ));
            }
            _ => panic!("expected WHERE clause"),
        }
    }

    #[test]
    fn test_order_by_limit_skip() {
        let stmt = parse("MATCH (n) RETURN n.name ORDER BY n.name DESC LIMIT 10 SKIP 5").unwrap();
        assert_eq!(stmt.clauses.len(), 5);
        assert!(matches!(&stmt.clauses[2], Clause::OrderBy(_)));
        assert!(matches!(&stmt.clauses[3], Clause::Limit(_)));
        assert!(matches!(&stmt.clauses[4], Clause::Skip(_)));
    }

    #[test]
    fn test_variable_length_path() {
        let stmt = parse("MATCH (a)-[:KNOWS*1..3]->(b) RETURN b").unwrap();
        match &stmt.clauses[0] {
            Clause::Match(m) => match &m.patterns[0].elements[1] {
                PatternElement::Relationship(r) => {
                    assert_eq!(r.length, Some((Some(1), Some(3))));
                }
                _ => panic!("expected relationship"),
            },
            _ => panic!("expected MATCH"),
        }
    }

    #[test]
    fn test_function_call() {
        let stmt =
            parse("MATCH (n) RETURN count(n), vector_similarity(n.embedding, $query)").unwrap();
        match &stmt.clauses[1] {
            Clause::Return(r) => {
                assert_eq!(r.items.len(), 2);
                assert!(
                    matches!(&r.items[0].expression, Expression::FunctionCall { name, .. } if name == "count")
                );
                assert!(
                    matches!(&r.items[1].expression, Expression::FunctionCall { name, .. } if name == "vector_similarity")
                );
            }
            _ => panic!("expected RETURN"),
        }
    }

    #[test]
    fn test_return_alias() {
        let stmt = parse("MATCH (n) RETURN n.name AS name").unwrap();
        match &stmt.clauses[1] {
            Clause::Return(r) => {
                assert_eq!(r.items[0].alias, Some("name".to_string()));
            }
            _ => panic!("expected RETURN"),
        }
    }

    #[test]
    fn test_create_edge() {
        let stmt = parse("CREATE (a)-[:KNOWS {since: 2020}]->(b)").unwrap();
        match &stmt.clauses[0] {
            Clause::Create(c) => {
                assert_eq!(c.patterns[0].elements.len(), 3);
                match &c.patterns[0].elements[1] {
                    PatternElement::Relationship(r) => {
                        assert_eq!(r.rel_types, vec!["KNOWS".to_string()]);
                        assert!(r.properties.is_some());
                    }
                    _ => panic!("expected relationship"),
                }
            }
            _ => panic!("expected CREATE"),
        }
    }

    #[test]
    fn test_set_property() {
        let stmt = parse("MATCH (n:Person) SET n.age = 31 RETURN n").unwrap();
        assert_eq!(stmt.clauses.len(), 3);
        match &stmt.clauses[1] {
            Clause::Set(s) => {
                assert_eq!(s.items.len(), 1);
            }
            _ => panic!("expected SET"),
        }
    }

    #[test]
    fn test_delete() {
        let stmt = parse("MATCH (n:Person) DELETE n").unwrap();
        match &stmt.clauses[1] {
            Clause::Delete(d) => {
                assert!(!d.detach);
                assert_eq!(d.expressions.len(), 1);
            }
            _ => panic!("expected DELETE"),
        }
    }

    #[test]
    fn test_detach_delete() {
        let stmt = parse("MATCH (n) DETACH DELETE n").unwrap();
        match &stmt.clauses[1] {
            Clause::Delete(d) => {
                assert!(d.detach);
            }
            _ => panic!("expected DETACH DELETE"),
        }
    }

    #[test]
    fn test_parameter_in_where() {
        let stmt = parse("MATCH (n) WHERE n.name = $name RETURN n").unwrap();
        match &stmt.clauses[1] {
            Clause::Where(w) => match &w.expression {
                Expression::Comparison { right, .. } => {
                    assert!(
                        matches!(right.as_ref(), Expression::Parameter(name) if name == "name")
                    );
                }
                _ => panic!("expected comparison"),
            },
            _ => panic!("expected WHERE"),
        }
    }

    #[test]
    fn test_return_distinct() {
        let stmt = parse("MATCH (n) RETURN DISTINCT n.name").unwrap();
        match &stmt.clauses[1] {
            Clause::Return(r) => {
                assert!(r.distinct);
            }
            _ => panic!("expected RETURN"),
        }
    }

    #[test]
    fn test_is_null() {
        let stmt = parse("MATCH (n) WHERE n.email IS NOT NULL RETURN n").unwrap();
        match &stmt.clauses[1] {
            Clause::Where(w) => {
                assert!(matches!(
                    &w.expression,
                    Expression::IsNull { negated: true, .. }
                ));
            }
            _ => panic!("expected WHERE"),
        }
    }

    #[test]
    fn test_combined_query() {
        // The PRD example query
        let stmt = parse(
            "MATCH (doc:Document)-[:REFERENCES]->(ref:Document) \
             WHERE doc.topic = 'AI' \
             RETURN ref, vector_similarity(ref.embedding, $query) AS sim \
             ORDER BY sim DESC \
             LIMIT 5",
        )
        .unwrap();
        assert_eq!(stmt.clauses.len(), 5);
        assert!(matches!(&stmt.clauses[0], Clause::Match(_)));
        assert!(matches!(&stmt.clauses[1], Clause::Where(_)));
        assert!(matches!(&stmt.clauses[2], Clause::Return(_)));
        assert!(matches!(&stmt.clauses[3], Clause::OrderBy(_)));
        assert!(matches!(&stmt.clauses[4], Clause::Limit(_)));
    }

    #[test]
    fn test_empty_query_error() {
        assert!(parse("").is_err());
    }

    #[test]
    fn test_invalid_token_error() {
        let result = parse("MATCH (n) FOOBAR n");
        assert!(result.is_err());
    }

    #[test]
    fn test_call_yield() {
        let stmt = parse("CALL vectrust.nearest('embedding', $query, 10) YIELD node, score RETURN node.name, score").unwrap();
        assert_eq!(stmt.clauses.len(), 2);
        match &stmt.clauses[0] {
            Clause::Call(c) => {
                assert_eq!(c.procedure, "vectrust.nearest");
                assert_eq!(c.args.len(), 3);
                assert_eq!(c.yields.len(), 2);
                assert_eq!(c.yields[0].name, "node");
                assert_eq!(c.yields[1].name, "score");
            }
            _ => panic!("expected CALL clause"),
        }
        assert!(matches!(&stmt.clauses[1], Clause::Return(_)));
    }
}
