use crate::compiler::analyzer::analyzed_expression::{
    AssignableExpression, AssignableExpressionKind,
};
use crate::compiler::analyzer::analyzed_type::AnalyzedTypeId;
use crate::compiler::analyzer::iterative_expression_analyzer::{
    analyze_expression, resolve_generic_type,
};
use crate::compiler::analyzer::program_analyzer::AnalyzerContext;
use crate::compiler::analyzer::AnalyzerResult;
use crate::compiler::lexer::location::Location;
use crate::compiler::parser::parsed_expression::{
    BinaryOp, ParsedExpression, ParsedExpressionKind, UnaryOp,
};
use anyhow::Context;

pub fn analyze_assignable_expression(
    context: &mut AnalyzerContext,
    expression: &ParsedExpression,
) -> AnalyzerResult<AssignableExpression> {
    match &expression.value {
        ParsedExpressionKind::Block { .. } => Err(anyhow::anyhow!(
            "Block expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::Return(_) => Err(anyhow::anyhow!(
            "Return expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::Continue => Err(anyhow::anyhow!(
            "Continue expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::Break(_) => Err(anyhow::anyhow!(
            "Break expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::If { .. } => Err(anyhow::anyhow!(
            "If expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::Loop { .. } => Err(anyhow::anyhow!(
            "Loop expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::Declaration { .. } => Err(anyhow::anyhow!(
            "Declaration expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::FunctionCall { .. } => Err(anyhow::anyhow!(
            "Function call expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::MemberFunctionCall { .. } => Err(anyhow::anyhow!(
            "Member function call expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::Literal(_) => Err(anyhow::anyhow!(
            "Literal expression cannot be assigned to at {}.",
            expression.location
        )),
        ParsedExpressionKind::Binary { op, left, right } => match op {
            BinaryOp::Index => {
                let analyzed_expr = analyze_expression(context, left).with_context(|| {
                    format!(
                        "Failed to analyze array expression at {}.",
                        expression.location
                    )
                })?;
                let analyzed_index = analyze_expression(context, right).with_context(|| {
                    format!(
                        "Failed to analyze array index expression at {}.",
                        expression.location
                    )
                })?;
                match &analyzed_index.ty {
                    AnalyzedTypeId::Integer(_) => {}
                    _ => Err(anyhow::anyhow!(
                        "Index expression has non-integer type '{}' at {}.",
                        analyzed_index.ty,
                        right.location
                    ))?,
                }
                match analyzed_expr.ty.clone() {
                    AnalyzedTypeId::Pointer(inner) => Ok(AssignableExpression {
                        ty: *inner,
                        kind: AssignableExpressionKind::ArrayIndex(
                            Box::new(analyzed_expr),
                            Box::new(analyzed_index),
                        ),
                    }),
                    _ => Err(anyhow::anyhow!(
                        "Index expression has non-array type '{}' at {}.",
                        analyzed_expr.ty,
                        left.location
                    )),
                }
            }
            _ => Err(anyhow::anyhow!(
                "Binary expression cannot be assigned to at {}.",
                expression.location
            )),
        },
        ParsedExpressionKind::Variable(name) => {
            let var_type = context.local_variables.get(name).ok_or_else(|| {
                anyhow::anyhow!(
                    "Variable '{}' not declared at {}.",
                    name,
                    expression.location
                )
            })?;
            Ok(AssignableExpression {
                kind: AssignableExpressionKind::LocalVariable(name.clone()),
                ty: var_type.ty.clone(),
            })
        }
        ParsedExpressionKind::Unary {
            op: UnaryOp::Dereference,
            expr,
        } => {
            let analyzed_expr = analyze_expression(context, expr).with_context(|| {
                format!(
                    "Failed to analyze assignable dereference expression at {}.",
                    expression.location
                )
            })?;
            match analyzed_expr.ty.clone() {
                AnalyzedTypeId::Pointer(inner) => Ok(AssignableExpression {
                    kind: AssignableExpressionKind::Dereference(Box::new(analyzed_expr)),
                    ty: *inner,
                }),
                _ => Err(anyhow::anyhow!(
                    "Dereference expression has non-pointer type '{}' at {}.",
                    analyzed_expr.ty,
                    expr.location
                )),
            }
        }
        ParsedExpressionKind::Unary {
            op: UnaryOp::Member(member),
            expr,
        } => try_as_assignable_field_access(context, member.clone(), expr, &expression.location),
        ParsedExpressionKind::Unary { .. } => Err(anyhow::anyhow!(
            "Unary expression cannot be assigned to at {}.",
            expression.location
        )),
    }
}

fn try_as_assignable_field_access(
    context: &mut AnalyzerContext,
    member: String,
    inner: &ParsedExpression,
    location: &Location,
) -> AnalyzerResult<AssignableExpression> {
    let maybe_assignable_expr = analyze_assignable_expression(context, inner);
    if let Ok(analyzed_expr) = maybe_assignable_expr {
        match &analyzed_expr.ty {
            AnalyzedTypeId::StructType(str_name, generic_args) => {
                let struct_type = context.types.get_struct(&analyzed_expr.ty).ok_or_else(|| {
                    anyhow::anyhow!("Struct type '{}' not found at {}.", str_name, location)
                })?;
                let field_type = struct_type.field_types.get(&member).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Struct type '{}' does not have field '{}' at {}.",
                        analyzed_expr.ty,
                        member,
                        location
                    )
                })?;
                let actual_field_type =
                    resolve_generic_type(field_type, &struct_type.generic_params, generic_args);
                return Ok(AssignableExpression {
                    kind: AssignableExpressionKind::FieldAccess(
                        Box::new(analyzed_expr),
                        member.clone(),
                    ),
                    ty: actual_field_type,
                });
            }
            AnalyzedTypeId::Pointer(_) => {}
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected struct type, found '{}' at {}.",
                    analyzed_expr.ty,
                    location
                ))?
            }
        }
    };

    let analyzed_expr = analyze_expression(context, inner).with_context(|| {
        format!(
            "Failed to analyze assignable member expression at {}.",
            location
        )
    })?;
    let mut inner_ty = &analyzed_expr.ty;
    let mut indirections = 0;
    while let AnalyzedTypeId::Pointer(inner) = inner_ty {
        inner_ty = inner;
        indirections += 1;
    }
    if indirections == 0 {
        return Err(anyhow::anyhow!(
            "Expected struct or struct pointer type, found '{}' at {}.",
            analyzed_expr.ty,
            location
        ))?;
    }

    if let AnalyzedTypeId::StructType(str_name, generic_args) = inner_ty {
        let struct_type = context.types.get_struct(inner_ty).ok_or_else(|| {
            anyhow::anyhow!("Struct type '{}' not found at {}.", str_name, location)
        })?;
        let field_type = struct_type.field_types.get(&member).ok_or_else(|| {
            anyhow::anyhow!(
                "Struct type '{}' does not have field '{}' at {}.",
                inner_ty,
                member,
                location
            )
        })?;
        let actual_field_type =
            resolve_generic_type(field_type, &struct_type.generic_params, generic_args);
        Ok(AssignableExpression {
            kind: AssignableExpressionKind::PointerFieldAccess(
                Box::new(analyzed_expr),
                member.clone(),
                indirections,
            ),
            ty: actual_field_type,
        })
    } else {
        Err(anyhow::anyhow!(
            "Expected struct type, found '{}' at {}.",
            analyzed_expr.ty,
            location
        ))?
    }
}

pub fn can_cast_to(original_type: &AnalyzedTypeId, target_type: &AnalyzedTypeId) -> bool {
    if *original_type == *target_type {
        return true;
    }
    match (original_type, target_type) {
        (AnalyzedTypeId::Integer(_), AnalyzedTypeId::Integer(_)) => true,
        (AnalyzedTypeId::Char, AnalyzedTypeId::Integer(_)) => true,
        (AnalyzedTypeId::Integer(_), AnalyzedTypeId::Char) => true,
        (AnalyzedTypeId::Bool, AnalyzedTypeId::Integer(_)) => true,
        (AnalyzedTypeId::Integer(_), AnalyzedTypeId::Bool) => true,
        (AnalyzedTypeId::Pointer(_), AnalyzedTypeId::Pointer(_)) => true,
        _ => false,
    }
}
