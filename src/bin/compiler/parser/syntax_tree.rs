use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lexer::{HasLocation, Location};
use crate::lexer::token::StaticToken;

#[derive(Debug)]
pub struct Program {
    pub functions: Vec<SrcFunction>,
}

#[derive(Debug, Clone)]
pub struct SrcFunction {
    pub(crate) function: Function,
    pub(crate) location: Location,
}

impl HasLocation for SrcFunction {
    fn location(&self) -> &Location {
        &self.location
    }
}

impl SrcFunction {
    pub fn new(function: Function, location: &Location) -> Self {
        Self {
            function,
            location: location.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub args: Vec<(String, Type)>,
    pub return_type: Option<Type>,
    pub statements: Vec<SrcStatement>,
}

#[derive(Debug, Clone)]
pub struct SrcStatement {
    pub(crate) statement: Statement,
    pub(crate) location: Location,
}

impl HasLocation for SrcStatement {
    fn location(&self) -> &Location {
        &self.location
    }
}

impl SrcStatement {
    pub fn new(statement: Statement, location: &Location) -> Self {
        Self {
            statement,
            location: location.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    Return(Option<SrcExpression>),
    Declaration {
        var_type: Type,
        name: String,
        value: Option<SrcExpression>,
    },
    Expr(SrcExpression),
}

#[derive(Debug, Clone)]
pub struct SrcExpression {
    pub(crate) expr: Expression,
    pub(crate) location: Location,
}

impl HasLocation for SrcExpression {
    fn location(&self) -> &Location {
        &self.location
    }
}

impl SrcExpression {
    pub fn new(expr: Expression, location: &Location) -> Self {
        Self {
            expr,
            location: location.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expression {
    Block(Vec<SrcStatement>, Option<Box<SrcExpression>>),
    Binary {
        op: BinaryOp,
        left: Box<SrcExpression>,
        right: Box<SrcExpression>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<SrcExpression>,
    },
    Literal(Literal),
    Variable(String),
    Ternary {
        condition: Box<SrcExpression>,
        true_expr: Box<SrcExpression>,
        false_expr: Box<SrcExpression>,
    },
    FunctionCall {
        function: String,
        args: Vec<SrcExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    LogicalAnd,
    LogicalOr,
    Shl,
    Shr,
    Equals,
    NotEquals,
    Less,
    LessEquals,
    Greater,
    GreaterEquals,
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    ShlAssign,
    ShrAssign,
}

lazy_static! {
    pub static ref ASSIGN_OP_MAP: HashMap<StaticToken, BinaryOp> = HashMap::from([
        (StaticToken::Assign, BinaryOp::Assign),
        (StaticToken::AddAssign, BinaryOp::AddAssign),
        (StaticToken::SubAssign, BinaryOp::SubAssign),
        (StaticToken::MulAssign, BinaryOp::MulAssign),
        (StaticToken::DivAssign, BinaryOp::DivAssign),
        (StaticToken::ModAssign, BinaryOp::ModAssign),
        (StaticToken::AndAssign, BinaryOp::AndAssign),
        (StaticToken::OrAssign, BinaryOp::OrAssign),
        (StaticToken::XorAssign, BinaryOp::XorAssign),
        (StaticToken::ShiftLeftAssign, BinaryOp::ShlAssign),
        (StaticToken::ShiftRightAssign, BinaryOp::ShrAssign),
    ]);
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Positive,
    Negate,
    Not,
    LogicalNot,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i32),
    Long(i64),
    String(String),
}

impl Literal {
    pub fn get_type(&self) -> Type {
        match self {
            Literal::Int(_) => Type::Int,
            Literal::Long(_) => Type::Long,
            Literal::String(_) => Type::String,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Bool,
    Int,
    Long,
    String,
}

impl Type {
    pub fn size(&self) -> usize {
        match self {
            Type::Unit => 0,
            Type::Bool => 1,
            Type::Int => 4,
            Type::Long => 8,
            Type::String => 8,
        }
    }

    pub fn is_integer(&self) -> bool {
        match self {
            Type::Int | Type::Long => true,
            _ => false,
        }
    }
}