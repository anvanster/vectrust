/// A complete Cypher statement (one or more clauses).
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub clauses: Vec<Clause>,
}

/// Individual clauses that make up a statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    Match(MatchClause),
    Create(CreateClause),
    Return(ReturnClause),
    Where(WhereClause),
    Set(SetClause),
    Delete(DeleteClause),
    Remove(RemoveClause),
    OrderBy(OrderByClause),
    Limit(LimitClause),
    Skip(SkipClause),
    With(WithClause),
    Call(CallClause),
    Merge(MergeClause),
}

// --- Clause types ---

#[derive(Debug, Clone, PartialEq)]
pub struct MatchClause {
    pub patterns: Vec<Pattern>,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateClause {
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnClause {
    pub distinct: bool,
    pub items: Vec<ReturnItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnItem {
    pub expression: Expression,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause {
    pub expression: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetClause {
    pub items: Vec<SetItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetItem {
    Property {
        target: Expression,
        value: Expression,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteClause {
    pub detach: bool,
    pub expressions: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoveClause {
    pub items: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderByClause {
    pub items: Vec<OrderItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderItem {
    pub expression: Expression,
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LimitClause {
    pub count: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkipClause {
    pub count: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithClause {
    pub distinct: bool,
    pub items: Vec<ReturnItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergeClause {
    pub pattern: Pattern,
    pub on_create: Vec<SetItem>,
    pub on_match: Vec<SetItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallClause {
    /// Procedure name (e.g., "vectrust.nearest")
    pub procedure: String,
    /// Arguments to the procedure
    pub args: Vec<Expression>,
    /// YIELD columns
    pub yields: Vec<YieldItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct YieldItem {
    pub name: String,
    pub alias: Option<String>,
}

// --- Pattern types ---

/// A pattern is a sequence of nodes connected by relationships.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub elements: Vec<PatternElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternElement {
    Node(NodePattern),
    Relationship(RelationshipPattern),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<MapLiteral>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelationshipPattern {
    pub variable: Option<String>,
    pub rel_types: Vec<String>,
    pub properties: Option<MapLiteral>,
    pub direction: Direction,
    /// Variable-length path: (min, max). None means single hop.
    pub length: Option<(Option<u32>, Option<u32>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    OutRight, // -[...]->
    InLeft,   // <-[...]-
    Both,     // -[...]-
}

// --- Expression types ---

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A literal value
    Literal(Literal),
    /// A variable reference: `n`
    Variable(String),
    /// Property access: `n.name`
    Property {
        object: Box<Expression>,
        key: String,
    },
    /// A query parameter: `$name`
    Parameter(String),
    /// Comparison: `a = b`, `a > b`, etc.
    Comparison {
        left: Box<Expression>,
        op: ComparisonOp,
        right: Box<Expression>,
    },
    /// Boolean AND / OR
    BoolOp {
        left: Box<Expression>,
        op: BooleanOp,
        right: Box<Expression>,
    },
    /// NOT expression
    Not(Box<Expression>),
    /// IS NULL / IS NOT NULL
    IsNull {
        expression: Box<Expression>,
        negated: bool,
    },
    /// Function call: `count(*)`, `vector_similarity(a, b)`
    FunctionCall {
        name: String,
        args: Vec<Expression>,
        distinct: bool,
    },
    /// List literal: `[1, 2, 3]`
    ListLiteral(Vec<Expression>),
    /// Map literal: `{key: value, ...}`
    MapLiteral(MapLiteral),
    /// String operators: CONTAINS, STARTS WITH, ENDS WITH, IN
    StringOp {
        left: Box<Expression>,
        op: StringMatchOp,
        right: Box<Expression>,
    },
    /// Arithmetic: `a + b`, `a * b`
    Arithmetic {
        left: Box<Expression>,
        op: ArithmeticOp,
        right: Box<Expression>,
    },
    /// Star expression (for count(*))
    Star,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapLiteral {
    pub entries: Vec<(String, Expression)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BooleanOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringMatchOp {
    Contains,
    StartsWith,
    EndsWith,
    In,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArithmeticOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}
