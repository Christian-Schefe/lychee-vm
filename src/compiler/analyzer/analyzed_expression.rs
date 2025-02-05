use crate::compiler::analyzer::analyzed_type::AnalyzedTypeId;
use crate::compiler::lexer::location::Location;
use crate::compiler::merger::merged_expression::{FunctionId, FunctionRef};
use crate::compiler::merger::resolved_functions::ResolvedFunctions;
use crate::compiler::merger::resolved_types::ResolvedTypes;
use crate::compiler::parser::binary_op::{BinaryComparisonOp, BinaryLogicOp, BinaryMathOp};
use crate::compiler::parser::parsed_expression::UnaryMathOp;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct AnalyzedProgram {
    pub resolved_types: ResolvedTypes,
    pub resolved_functions: ResolvedFunctions,
    pub function_bodies: HashMap<FunctionId, AnalyzedExpression>,
    pub main_function: FunctionRef,
}

#[derive(Debug, Clone)]
pub struct AnalyzedExpression {
    pub kind: AnalyzedExpressionKind,
    pub ty: AnalyzedTypeId,
    pub location: Location,
}

#[derive(Debug, Clone)]
pub enum AnalyzedExpressionKind {
    Block {
        expressions: Vec<AnalyzedExpression>,
        returns_value: bool,
    },
    Return(Option<Box<AnalyzedExpression>>),
    Continue,
    Break(Option<Box<AnalyzedExpression>>),
    If {
        condition: Box<AnalyzedExpression>,
        then_block: Box<AnalyzedExpression>,
        else_expr: Option<Box<AnalyzedExpression>>,
    },
    Loop {
        init: Option<Box<AnalyzedExpression>>,
        condition: Option<Box<AnalyzedExpression>>,
        step: Option<Box<AnalyzedExpression>>,
        loop_body: Box<AnalyzedExpression>,
        else_expr: Option<Box<AnalyzedExpression>>,
    },
    Declaration {
        var_name: String,
        value: Box<AnalyzedExpression>,
    },
    ValueOfAssignable(AssignableExpression),
    Literal(AnalyzedLiteral),
    ConstantPointer(AnalyzedConstant),
    Unary {
        op: AnalyzedUnaryOp,
        expr: Box<AnalyzedExpression>,
    },
    Binary {
        op: AnalyzedBinaryOp,
        left: Box<AnalyzedExpression>,
        right: Box<AnalyzedExpression>,
    },
    Assign {
        op: BinaryAssignOp,
        lhs: AssignableExpression,
        rhs: Box<AnalyzedExpression>,
    },
    Borrow {
        expr: AssignableExpression,
    },
    FunctionCall {
        call_type: AnalyzedFunctionCallType,
        args: Vec<AnalyzedExpression>,
    },
    FieldAccess {
        expr: Box<AnalyzedExpression>,
        field_name: String,
    },
    Increment(AssignableExpression, bool),
    Decrement(AssignableExpression, bool),
    Sizeof(AnalyzedTypeId),
    StructInstance {
        fields: Vec<(String, AnalyzedExpression)>,
    },
    FunctionPointer(FunctionRef),
}

#[derive(Debug, Clone)]
pub enum AnalyzedFunctionCallType {
    Function(FunctionRef),
    FunctionPointer(Box<AnalyzedExpression>),
}

#[derive(Debug, Clone)]
pub enum AnalyzedLiteral {
    Unit,
    Bool(bool),
    Char(i8),
    Integer(i64),
}

#[derive(Debug, Clone)]
pub enum AnalyzedConstant {
    String(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct AssignableExpression {
    pub kind: AssignableExpressionKind,
    pub ty: AnalyzedTypeId,
}
#[derive(Debug, Clone)]
pub enum AssignableExpressionKind {
    LocalVariable(String),
    Dereference(Box<AnalyzedExpression>),
    FieldAccess(Box<AssignableExpression>, String),
    PointerFieldAccess(Box<AnalyzedExpression>, String, usize),
    ArrayIndex(Box<AnalyzedExpression>, Box<AnalyzedExpression>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzedBinaryOp {
    Math(BinaryMathOp),
    Logical(BinaryLogicOp),
    Comparison(BinaryComparisonOp),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryAssignOp {
    Assign,
    MathAssign(BinaryMathOp),
    LogicAssign(BinaryLogicOp),
}

#[derive(Debug, Clone)]
pub enum AnalyzedUnaryOp {
    Math(UnaryMathOp),
    LogicalNot,
    Cast,
}
