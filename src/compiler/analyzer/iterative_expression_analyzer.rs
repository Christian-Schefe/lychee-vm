use crate::compiler::analyzer::analyzed_expression::{
    AnalyzedBinaryOp, AnalyzedConstant, AnalyzedExpression, AnalyzedExpressionKind,
    AnalyzedFunctionCallType, AnalyzedLiteral, AnalyzedUnaryOp, AssignableExpression,
    AssignableExpressionKind, BinaryAssignOp,
};
use crate::compiler::analyzer::analyzed_type::{AnalyzedTypeId, GenericId, GenericParams};
use crate::compiler::analyzer::expression_analyzer::{analyze_assignable_expression, can_cast_to};
use crate::compiler::analyzer::program_analyzer::{AnalyzerContext, LocalVariable};
use crate::compiler::analyzer::AnalyzerResult;
use crate::compiler::lexer::location::Location;
use crate::compiler::merger::merged_expression::StructRef;
use crate::compiler::parser::binary_op::{BinaryComparisonOp, BinaryOp};
use crate::compiler::parser::item_id::ParsedGenericId;
use crate::compiler::parser::parsed_expression::{
    ParsedExpression, ParsedExpressionKind, ParsedLiteral, ParsedType, UnaryOp,
};
use anyhow::Context;
use std::collections::HashMap;

pub fn analyze_expression(
    context: &mut AnalyzerContext,
    expression: &ParsedExpression,
) -> AnalyzerResult<AnalyzedExpression> {
    let mut local_var_stack = vec![];
    let mut stack = vec![(expression, false, false, None)];
    let mut output: Vec<AnalyzedExpression> = vec![];

    while let Some((stack_expr, was_visited, in_loop, type_hint)) = stack.pop() {
        let location = stack_expr.location.clone();
        if !was_visited {
            stack.push((stack_expr, true, in_loop, type_hint));
            match &stack_expr.value {
                ParsedExpressionKind::Block { expressions, .. } => {
                    local_var_stack.push(context.local_variables.clone());
                    context
                        .local_variables
                        .values_mut()
                        .for_each(|v| v.is_current_scope = false);
                    for expression in expressions.iter().rev() {
                        stack.push((expression, false, in_loop, None));
                    }
                }
                ParsedExpressionKind::Return(maybe_expr) => {
                    if let Some(expr) = maybe_expr {
                        stack.push((expr, false, in_loop, None));
                    }
                }
                ParsedExpressionKind::Continue => {}
                ParsedExpressionKind::Break(maybe_expr) => {
                    if let Some(expr) = maybe_expr {
                        stack.push((expr, false, in_loop, None));
                    }
                }
                ParsedExpressionKind::If {
                    condition,
                    then_block,
                    else_expr,
                } => {
                    if let Some(else_expr) = else_expr {
                        stack.push((else_expr, false, in_loop, None));
                    }
                    stack.push((then_block, false, in_loop, None));
                    stack.push((condition, false, in_loop, None));
                }
                ParsedExpressionKind::Loop {
                    init,
                    condition,
                    step,
                    loop_body,
                    else_expr,
                } => {
                    if let Some(else_expr) = else_expr {
                        stack.push((else_expr, false, in_loop, None));
                    }
                    stack.push((loop_body, false, true, None));
                    if let Some(step) = step {
                        stack.push((step, false, false, None));
                    }
                    if let Some(condition) = condition {
                        stack.push((condition, false, false, None));
                    }
                    if let Some(init) = init {
                        stack.push((init, false, in_loop, None));
                    }
                }
                ParsedExpressionKind::Declaration {
                    value, var_type, ..
                } => {
                    let resolved_type_hint = var_type
                        .as_ref()
                        .map(|x| {
                            context
                                .types
                                .map_generic_parsed_type(&x.value, context.generic_params)
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Declaration type '{}' not found at {}.",
                                        x.value,
                                        x.location
                                    )
                                })
                        })
                        .transpose()?;
                    stack.push((value, false, in_loop, resolved_type_hint));
                }
                ParsedExpressionKind::Variable(_) => {}
                ParsedExpressionKind::StructInstance {
                    struct_type: ty,
                    fields,
                } => {
                    let resolved_type = context
                        .types
                        .map_generic_parsed_type(&ty.value, context.generic_params)
                        .ok_or_else(|| {
                            anyhow::anyhow!("Type '{}' not found at {}.", ty.value, ty.location)
                        })?;
                    let struct_decl = context
                        .types
                        .get_struct_from_type(&resolved_type)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Type '{}' is not a struct type at {}.",
                                resolved_type,
                                ty.location
                            )
                        })?;
                    let mut present_fields = fields
                        .iter()
                        .map(|x| (&x.0, &x.1))
                        .collect::<HashMap<_, _>>();
                    for field in struct_decl.field_order.iter().rev() {
                        if let Some(value) = present_fields.get(field) {
                            stack.push((value, false, in_loop, None));
                            present_fields.remove(field);
                        } else {
                            Err(anyhow::anyhow!(
                                "Struct field '{}' missing at {}.",
                                field,
                                location
                            ))?;
                        }
                    }
                    if !present_fields.is_empty() {
                        Err(anyhow::anyhow!(
                            "Struct has extra fields {:?} at {}.",
                            present_fields.keys(),
                            location
                        ))?;
                    }
                }
                ParsedExpressionKind::Literal(_) => {}
                ParsedExpressionKind::Unary { op, expr } => {
                    let expr_is_assignable = match op {
                        UnaryOp::Increment { .. } | UnaryOp::Decrement { .. } | UnaryOp::Borrow => {
                            true
                        }
                        _ => false,
                    };
                    if !expr_is_assignable {
                        stack.push((expr, false, in_loop, None));
                    }
                }
                ParsedExpressionKind::Binary { left, right, op } => {
                    let left_is_assignable = match op {
                        BinaryOp::Assign | BinaryOp::MathAssign(_) | BinaryOp::LogicAssign(_) => {
                            true
                        }
                        _ => false,
                    };
                    stack.push((right, false, in_loop, None));
                    if !left_is_assignable {
                        stack.push((left, false, in_loop, None));
                    }
                }
                ParsedExpressionKind::FunctionCall { args, .. } => {
                    for arg in args.iter().rev() {
                        stack.push((arg, false, in_loop, None));
                    }
                }
                ParsedExpressionKind::Sizeof(_) => {}
                ParsedExpressionKind::Tuple(elements) => {
                    for element in elements.iter().rev() {
                        stack.push((element, false, in_loop, None));
                    }
                }
            }
        } else {
            let (ty, analyzed) = match &stack_expr.value {
                ParsedExpressionKind::Block {
                    expressions,
                    returns_value,
                } => {
                    context.local_variables = local_var_stack.pop().unwrap();
                    let new_len = output.len() - expressions.len();
                    let analyzed_expressions = output.split_off(new_len);
                    let return_ty = analyzed_expressions
                        .last()
                        .filter(|_| *returns_value)
                        .map(|a| a.ty.clone())
                        .unwrap_or(AnalyzedTypeId::Unit);
                    (
                        return_ty,
                        AnalyzedExpressionKind::Block {
                            expressions: analyzed_expressions,
                            returns_value: *returns_value,
                        },
                    )
                }
                ParsedExpressionKind::Return(maybe_expr) => {
                    let analyzed_expr =
                        maybe_expr.as_ref().map(|_| Box::new(output.pop().unwrap()));
                    let return_ty = analyzed_expr
                        .as_ref()
                        .map_or(AnalyzedTypeId::Unit, |e| e.ty.clone());
                    if return_ty != *context.return_type {
                        Err(anyhow::anyhow!(
                            "Return type '{}' does not match function return type '{}' at {}.",
                            return_ty,
                            context.return_type,
                            location
                        ))?;
                    }
                    (
                        AnalyzedTypeId::Unit,
                        AnalyzedExpressionKind::Return(analyzed_expr),
                    )
                }
                ParsedExpressionKind::Continue => {
                    if !in_loop {
                        Err(anyhow::anyhow!("Continue outside of loop at {}.", location))?;
                    }
                    (AnalyzedTypeId::Unit, AnalyzedExpressionKind::Continue)
                }
                ParsedExpressionKind::Break(maybe_expr) => {
                    if !in_loop {
                        Err(anyhow::anyhow!("Break outside of loop at {}.", location))?;
                    }
                    let analyzed_expr =
                        maybe_expr.as_ref().map(|_| Box::new(output.pop().unwrap()));
                    (
                        AnalyzedTypeId::Unit,
                        AnalyzedExpressionKind::Break(analyzed_expr),
                    )
                }
                ParsedExpressionKind::If { else_expr, .. } => {
                    let analyzed_else = else_expr.as_ref().map(|_| Box::new(output.pop().unwrap()));
                    let analyzed_then = output.pop().unwrap();
                    let analyzed_condition = output.pop().unwrap();
                    if analyzed_condition.ty != AnalyzedTypeId::Bool {
                        Err(anyhow::anyhow!(
                            "If condition has non-bool type at {}.",
                            location
                        ))?;
                    }
                    let return_ty = if let Some(else_expr) = &analyzed_else {
                        if analyzed_then.ty != else_expr.ty {
                            Err(anyhow::anyhow!(
                                "If then block has type '{}', but else block has type '{}' at {}.",
                                analyzed_then.ty,
                                else_expr.ty,
                                location
                            ))?;
                        }
                        analyzed_then.ty.clone()
                    } else {
                        if analyzed_then.ty != AnalyzedTypeId::Unit {
                            Err(anyhow::anyhow!(
                                "If then block has non-unit type '{}' but else block is missing at {}.",
                                analyzed_then.ty,
                                location
                            ))?;
                        }
                        AnalyzedTypeId::Unit
                    };
                    (
                        return_ty,
                        AnalyzedExpressionKind::If {
                            condition: Box::new(analyzed_condition),
                            then_block: Box::new(analyzed_then),
                            else_expr: analyzed_else,
                        },
                    )
                }
                ParsedExpressionKind::Loop {
                    init,
                    condition,
                    step,
                    loop_body: _,
                    else_expr,
                } => {
                    let analyzed_else = else_expr.as_ref().map(|_| Box::new(output.pop().unwrap()));
                    let analyzed_loop_body = output.pop().unwrap();
                    let analyzed_step = step.as_ref().map(|_| Box::new(output.pop().unwrap()));
                    let analyzed_condition =
                        condition.as_ref().map(|_| Box::new(output.pop().unwrap()));
                    let analyzed_init = init.as_ref().map(|_| Box::new(output.pop().unwrap()));
                    if analyzed_init
                        .as_ref()
                        .is_some_and(|x| x.ty != AnalyzedTypeId::Unit)
                    {
                        Err(anyhow::anyhow!(
                            "Loop init has non-unit type '{}' at {}.",
                            analyzed_init.as_ref().unwrap().ty,
                            location
                        ))?;
                    }
                    if analyzed_condition
                        .as_ref()
                        .is_some_and(|x| x.ty != AnalyzedTypeId::Bool)
                    {
                        Err(anyhow::anyhow!(
                            "Loop condition has non-bool type '{}' at {}.",
                            analyzed_condition.as_ref().unwrap().ty,
                            location
                        ))?;
                    }
                    match analyzed_step
                        .as_ref()
                        .map(|x| x.ty.clone())
                        .unwrap_or(AnalyzedTypeId::Unit)
                    {
                        AnalyzedTypeId::Unit => {}
                        AnalyzedTypeId::Integer(_) => {}
                        any_ty => Err(anyhow::anyhow!(
                            "Loop step has non-unit/non-integer type '{}' at {}.",
                            any_ty,
                            location
                        ))?,
                    }
                    if analyzed_loop_body.ty != AnalyzedTypeId::Unit {
                        Err(anyhow::anyhow!(
                            "Loop body has non-unit type '{}' at {}.",
                            analyzed_loop_body.ty,
                            location
                        ))?;
                    }
                    let else_ty = analyzed_else.as_ref().map(|e| e.ty.clone());

                    let has_condition = analyzed_condition.is_some();
                    let has_else = analyzed_else.is_some();

                    let required_break_type = match (has_condition, has_else) {
                        (true, true) => Some(else_ty.unwrap()),
                        (true, false) => Some(AnalyzedTypeId::Unit),
                        (false, true) => unreachable!("Else without condition"),
                        (false, false) => None,
                    };

                    let final_return_ty = assert_break_return_type(
                        required_break_type.as_ref(),
                        &analyzed_loop_body,
                    )?
                    .unwrap_or(AnalyzedTypeId::Unit);
                    (
                        final_return_ty,
                        AnalyzedExpressionKind::Loop {
                            init: analyzed_init,
                            condition: analyzed_condition,
                            step: analyzed_step,
                            loop_body: Box::new(analyzed_loop_body),
                            else_expr: analyzed_else,
                        },
                    )
                }
                ParsedExpressionKind::Declaration {
                    value,
                    var_type,
                    var_name,
                } => {
                    let analyzed_value = output.pop().unwrap();
                    if let Some(declared_type) = var_type {
                        let resolved_type = context
                            .types
                            .map_generic_parsed_type(&declared_type.value, context.generic_params)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Declaration type '{}' not found at {}.",
                                    declared_type.value,
                                    declared_type.location
                                )
                            })?;
                        if analyzed_value.ty != resolved_type {
                            Err(anyhow::anyhow!(
                                "Declaration expression should be of type '{}', but was '{}' at {}.",
                                resolved_type,
                                analyzed_value.ty,
                                value.location
                            ))?;
                        }
                    }
                    if let Some(old_var) = context.local_variables.insert(
                        var_name.clone(),
                        LocalVariable {
                            ty: analyzed_value.ty.clone(),
                            is_current_scope: true,
                        },
                    ) {
                        if old_var.is_current_scope {
                            Err(anyhow::anyhow!(
                                "Variable '{}' already declared at {}.",
                                var_name,
                                location
                            ))?;
                        }
                    }
                    (
                        AnalyzedTypeId::Unit,
                        AnalyzedExpressionKind::Declaration {
                            var_name: var_name.clone(),
                            value: Box::new(analyzed_value),
                        },
                    )
                }
                ParsedExpressionKind::Variable(var_name) => {
                    determine_variable_expression(context, var_name, &location, type_hint)?
                }
                ParsedExpressionKind::Literal(lit) => match lit {
                    ParsedLiteral::Unit => (
                        AnalyzedTypeId::Unit,
                        AnalyzedExpressionKind::Literal(AnalyzedLiteral::Unit),
                    ),
                    ParsedLiteral::Bool(b) => (
                        AnalyzedTypeId::Bool,
                        AnalyzedExpressionKind::Literal(AnalyzedLiteral::Bool(*b)),
                    ),
                    ParsedLiteral::Char(c) => (
                        AnalyzedTypeId::Char,
                        AnalyzedExpressionKind::Literal(AnalyzedLiteral::Char(*c)),
                    ),
                    ParsedLiteral::Integer(val) => {
                        let ty = if *val >= -2147483648 && *val <= 2147483647 {
                            AnalyzedTypeId::Integer(4)
                        } else {
                            AnalyzedTypeId::Integer(8)
                        };
                        (
                            ty,
                            AnalyzedExpressionKind::Literal(AnalyzedLiteral::Integer(*val)),
                        )
                    }
                    ParsedLiteral::String(val) => {
                        let mut bytes = val.as_bytes().to_vec();
                        bytes.push(0);
                        (
                            AnalyzedTypeId::Pointer(Box::new(AnalyzedTypeId::Char)),
                            AnalyzedExpressionKind::ConstantPointer(AnalyzedConstant::String(
                                bytes,
                            )),
                        )
                    }
                },
                ParsedExpressionKind::StructInstance {
                    struct_type: ty,
                    fields: field_values,
                } => {
                    let resolved_type = context
                        .types
                        .map_generic_parsed_type(&ty.value, context.generic_params)
                        .unwrap();
                    let struct_type = context.types.get_struct_from_type(&resolved_type).unwrap();

                    let struct_ref = match &resolved_type {
                        AnalyzedTypeId::StructType(struct_ref) => struct_ref,
                        _ => {
                            return Err(anyhow::anyhow!(
                                "Resolved type '{}' is not a struct at {}.",
                                resolved_type,
                                location
                            ))?
                        }
                    };

                    let mut analyzed_field_values = Vec::new();
                    let location_map = field_values
                        .iter()
                        .map(|(k, v)| (k.clone(), &v.location))
                        .collect::<HashMap<_, _>>();

                    for field_name in struct_type.field_order.iter().rev() {
                        let analyzed_field_value = output.pop().unwrap();
                        let expected_type = struct_type
                            .get_field_type(field_name, &struct_ref.generic_args)
                            .unwrap();
                        if analyzed_field_value.ty != expected_type {
                            Err(anyhow::anyhow!(
                                "Struct field '{}' has type '{}', but expected '{}' at {}.",
                                field_name,
                                analyzed_field_value.ty,
                                expected_type,
                                location_map.get(field_name).unwrap()
                            ))?;
                        }
                        analyzed_field_values.push((field_name.clone(), analyzed_field_value));
                    }

                    analyzed_field_values.reverse();

                    (
                        resolved_type,
                        AnalyzedExpressionKind::StructInstance {
                            fields: analyzed_field_values,
                        },
                    )
                }
                ParsedExpressionKind::Unary { expr, op } => match op {
                    UnaryOp::Math(math_op) => {
                        let analyzed_expr = output.pop().unwrap();
                        match analyzed_expr.ty {
                            AnalyzedTypeId::Integer(_) => {}
                            _ => Err(anyhow::anyhow!(
                                "Math unary expression has non-integer type '{}' at {}.",
                                analyzed_expr.ty,
                                location
                            ))?,
                        }

                        (
                            analyzed_expr.ty.clone(),
                            AnalyzedExpressionKind::Unary {
                                op: AnalyzedUnaryOp::Math(math_op.clone()),
                                expr: Box::new(analyzed_expr),
                            },
                        )
                    }
                    UnaryOp::LogicalNot => {
                        let analyzed_expr = output.pop().unwrap();
                        if analyzed_expr.ty != AnalyzedTypeId::Bool {
                            Err(anyhow::anyhow!(
                                "Logical not expression has non-bool type '{}' at {}.",
                                analyzed_expr.ty,
                                location
                            ))?;
                        }

                        (
                            AnalyzedTypeId::Bool,
                            AnalyzedExpressionKind::Unary {
                                op: AnalyzedUnaryOp::LogicalNot,
                                expr: Box::new(analyzed_expr),
                            },
                        )
                    }
                    UnaryOp::Borrow => {
                        let analyzed_expr = analyze_assignable_expression(context, expr)
                            .with_context(|| {
                                format!(
                                    "Failed to analyze type of borrow expression at {}.",
                                    location
                                )
                            })?;
                        (
                            AnalyzedTypeId::Pointer(Box::new(analyzed_expr.ty.clone())),
                            AnalyzedExpressionKind::Borrow {
                                expr: analyzed_expr,
                            },
                        )
                    }
                    UnaryOp::Dereference => {
                        let analyzed_expr = output.pop().unwrap();
                        match analyzed_expr.ty.clone() {
                            AnalyzedTypeId::Pointer(inner) => (
                                *inner.clone(),
                                AnalyzedExpressionKind::ValueOfAssignable(AssignableExpression {
                                    kind: AssignableExpressionKind::Dereference(Box::new(
                                        analyzed_expr,
                                    )),
                                    ty: *inner,
                                }),
                            ),
                            _ => Err(anyhow::anyhow!(
                                "Dereference expression has non-pointer type '{}' at {}.",
                                analyzed_expr.ty,
                                location
                            ))?,
                        }
                    }
                    UnaryOp::Increment { is_prefix } => {
                        let analyzed_expr = analyze_assignable_expression(context, expr)
                            .with_context(|| {
                                format!(
                                    "Failed to analyze type of increment expression at {}.",
                                    location
                                )
                            })?;
                        match &analyzed_expr.ty {
                            AnalyzedTypeId::Integer(_) => {}
                            _ => Err(anyhow::anyhow!(
                                "Increment expression has non-integer type '{}' at {}.",
                                analyzed_expr.ty,
                                location
                            ))?,
                        }
                        (
                            analyzed_expr.ty.clone(),
                            AnalyzedExpressionKind::Increment(analyzed_expr, *is_prefix),
                        )
                    }
                    UnaryOp::Decrement { is_prefix } => {
                        let analyzed_expr = analyze_assignable_expression(context, expr)
                            .with_context(|| {
                                format!(
                                    "Failed to analyze type of decrement expression at {}.",
                                    location
                                )
                            })?;
                        match &analyzed_expr.ty {
                            AnalyzedTypeId::Integer(_) => {}
                            _ => Err(anyhow::anyhow!(
                                "Decrement expression has non-integer type '{}' at {}.",
                                analyzed_expr.ty,
                                location
                            ))?,
                        }
                        (
                            analyzed_expr.ty.clone(),
                            AnalyzedExpressionKind::Decrement(analyzed_expr, *is_prefix),
                        )
                    }
                    UnaryOp::Cast(target_type) => {
                        let resolved_type = context
                            .types
                            .map_generic_parsed_type(&target_type.value, context.generic_params)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Cast type '{:?}' not found at {}.",
                                    target_type.value,
                                    target_type.location
                                )
                            })?;
                        let analyzed_expr = output.pop().unwrap();
                        if can_cast_to(&analyzed_expr.ty, &resolved_type) {
                            (
                                resolved_type,
                                AnalyzedExpressionKind::Unary {
                                    op: AnalyzedUnaryOp::Cast,
                                    expr: Box::new(analyzed_expr),
                                },
                            )
                        } else {
                            Err(anyhow::anyhow!(
                                "Cast expression has type '{}', but expected '{}' at {}.",
                                analyzed_expr.ty,
                                resolved_type,
                                location
                            ))?
                        }
                    }
                    UnaryOp::Member(member) => {
                        let analyzed_expr = output.pop().unwrap();
                        let mut inner_ty = &analyzed_expr.ty;
                        let mut indirections = 0;
                        while let AnalyzedTypeId::Pointer(inner) = inner_ty {
                            inner_ty = inner;
                            indirections += 1;
                        }
                        let struct_type =
                            context
                                .types
                                .get_struct_from_type(inner_ty)
                                .ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Struct type '{}' not found at {}.",
                                        inner_ty,
                                        expr.location
                                    )
                                })?;
                        let struct_ref = match &inner_ty {
                            AnalyzedTypeId::StructType(struct_ref) => struct_ref,
                            _ => {
                                return Err(anyhow::anyhow!(
                                    "Resolved type '{}' is not a struct at {}.",
                                    inner_ty,
                                    location
                                ))?
                            }
                        };
                        let field_type = struct_type
                            .get_field_type(member, &struct_ref.generic_args)
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "Struct type '{}' does not have field '{}' at {}.",
                                    analyzed_expr.ty,
                                    member,
                                    expr.location
                                )
                            })?;
                        if indirections == 0 {
                            (
                                field_type.clone(),
                                AnalyzedExpressionKind::FieldAccess {
                                    field_name: member.clone(),
                                    expr: Box::new(analyzed_expr),
                                },
                            )
                        } else {
                            (
                                field_type.clone(),
                                AnalyzedExpressionKind::ValueOfAssignable(AssignableExpression {
                                    kind: AssignableExpressionKind::PointerFieldAccess(
                                        Box::new(analyzed_expr),
                                        member.clone(),
                                        1,
                                    ),
                                    ty: field_type.clone(),
                                }),
                            )
                        }
                    }
                },
                ParsedExpressionKind::Binary { left, op, right } => match op {
                    BinaryOp::Index => {
                        let analyzed_index = output.pop().unwrap();
                        let analyzed_expr = output.pop().unwrap();
                        match &analyzed_index.ty {
                            AnalyzedTypeId::Integer(_) => {}
                            _ => Err(anyhow::anyhow!(
                                "Index expression has non-integer type '{}' at {}.",
                                analyzed_index.ty,
                                right.location
                            ))?,
                        }
                        match analyzed_expr.ty.clone() {
                            AnalyzedTypeId::Pointer(inner) => (
                                *inner.clone(),
                                AnalyzedExpressionKind::ValueOfAssignable(AssignableExpression {
                                    kind: AssignableExpressionKind::ArrayIndex(
                                        Box::new(analyzed_expr),
                                        Box::new(analyzed_index),
                                    ),
                                    ty: *inner,
                                }),
                            ),
                            _ => Err(anyhow::anyhow!(
                                "Index expression has non-array type '{}' at {}.",
                                analyzed_expr.ty,
                                left.location
                            ))?,
                        }
                    }
                    BinaryOp::Math(math_op) => {
                        let analyzed_right = output.pop().unwrap();
                        let analyzed_left = output.pop().unwrap();
                        match &analyzed_left.ty {
                            AnalyzedTypeId::Integer(_) => {}
                            _ => Err(anyhow::anyhow!(
                                "Math binary left expression has non-integer type '{}' at {}.",
                                analyzed_left.ty,
                                left.location
                            ))?,
                        }
                        if analyzed_left.ty != analyzed_right.ty {
                            Err(anyhow::anyhow!("Math binary left expression has type '{}', but right expression has type '{}' at {}.", analyzed_left.ty, analyzed_right.ty, right.location))?;
                        }

                        (
                            analyzed_left.ty.clone(),
                            AnalyzedExpressionKind::Binary {
                                op: AnalyzedBinaryOp::Math(math_op.clone()),
                                left: Box::new(analyzed_left),
                                right: Box::new(analyzed_right),
                            },
                        )
                    }
                    BinaryOp::Logical(logic_op) => {
                        let analyzed_right = output.pop().unwrap();
                        let analyzed_left = output.pop().unwrap();
                        if analyzed_left.ty != AnalyzedTypeId::Bool {
                            Err(anyhow::anyhow!(
                                "Logic binary left expression has non-bool type '{}' at {}.",
                                analyzed_left.ty,
                                left.location
                            ))?;
                        }
                        if analyzed_right.ty != AnalyzedTypeId::Bool {
                            Err(anyhow::anyhow!(
                                "Logic binary right expression has non-bool type '{}' at {}.",
                                analyzed_right.ty,
                                right.location
                            ))?;
                        }

                        (
                            AnalyzedTypeId::Bool,
                            AnalyzedExpressionKind::Binary {
                                op: AnalyzedBinaryOp::Logical(logic_op.clone()),
                                left: Box::new(analyzed_left),
                                right: Box::new(analyzed_right),
                            },
                        )
                    }
                    BinaryOp::Comparison(comp_op) => {
                        let analyzed_right = output.pop().unwrap();
                        let analyzed_left = output.pop().unwrap();
                        let needs_integers = match comp_op {
                            BinaryComparisonOp::Equals | BinaryComparisonOp::NotEquals => false,
                            _ => true,
                        };
                        match &analyzed_left.ty {
                            AnalyzedTypeId::Integer(_) => {}
                            AnalyzedTypeId::Char
                            | AnalyzedTypeId::Bool
                            | AnalyzedTypeId::EnumType(_)
                                if !needs_integers => {}
                            _ => Err(anyhow::anyhow!(
                            "Comparison binary left expression has non-comparable type '{}' at {}.",
                            analyzed_left.ty,
                            left.location
                        ))?,
                        }
                        if analyzed_left.ty != analyzed_right.ty {
                            Err(anyhow::anyhow!("Comparison binary left expression has type '{}', but right expression has type '{}' at {}.", analyzed_left.ty, analyzed_right.ty, right.location))?;
                        }

                        (
                            AnalyzedTypeId::Bool,
                            AnalyzedExpressionKind::Binary {
                                op: AnalyzedBinaryOp::Comparison(comp_op.clone()),
                                left: Box::new(analyzed_left),
                                right: Box::new(analyzed_right),
                            },
                        )
                    }
                    BinaryOp::Assign => {
                        let analyzed_right = output.pop().unwrap();
                        let analyzed_left = analyze_assignable_expression(context, left)
                            .with_context(|| {
                                format!(
                                    "Failed to analyze type of assign binary left expression at {}.",
                                    location
                                )
                            })?;
                        if analyzed_left.ty != analyzed_right.ty {
                            Err(anyhow::anyhow!("Assign binary left expression has type '{}', but right expression has type '{}' at {}.", analyzed_left.ty, analyzed_right.ty, right.location))?;
                        }
                        (
                            analyzed_left.ty.clone(),
                            AnalyzedExpressionKind::Assign {
                                op: BinaryAssignOp::Assign,
                                lhs: analyzed_left,
                                rhs: Box::new(analyzed_right),
                            },
                        )
                    }
                    BinaryOp::MathAssign(math_op) => {
                        let analyzed_right = output.pop().unwrap();
                        let analyzed_left = analyze_assignable_expression(context, left).with_context(|| format!("Failed to analyze type of math assign binary left expression at {}.", location))?;

                        match &analyzed_left.ty {
                            AnalyzedTypeId::Integer(_) => {}
                            _ => Err(anyhow::anyhow!(
                            "Math assign binary left expression has non-integer type '{}' at {}.",
                            analyzed_left.ty,
                            left.location
                        ))?,
                        }
                        if analyzed_left.ty != analyzed_right.ty {
                            Err(anyhow::anyhow!("Math assign binary left expression has type '{}', but right expression has type '{}' at {}.", analyzed_left.ty, analyzed_right.ty, right.location))?;
                        }

                        (
                            analyzed_left.ty.clone(),
                            AnalyzedExpressionKind::Assign {
                                op: BinaryAssignOp::MathAssign(math_op.clone()),
                                lhs: analyzed_left,
                                rhs: Box::new(analyzed_right),
                            },
                        )
                    }
                    BinaryOp::LogicAssign(logic_op) => {
                        let analyzed_right = output.pop().unwrap();
                        let analyzed_left = analyze_assignable_expression(context, left).with_context(|| format!("Failed to analyze type of logic assign binary left expression at {}.", location))?;
                        if analyzed_left.ty != AnalyzedTypeId::Bool {
                            Err(anyhow::anyhow!(
                                "Logic assign binary left expression has non-bool type '{}' at {}.",
                                analyzed_left.ty,
                                left.location
                            ))?;
                        }
                        if analyzed_right.ty != AnalyzedTypeId::Bool {
                            Err(anyhow::anyhow!(
                            "Logic assign binary right expression has non-bool type '{}' at {}.",
                            analyzed_right.ty,
                            right.location
                        ))?;
                        }

                        (
                            AnalyzedTypeId::Bool,
                            AnalyzedExpressionKind::Assign {
                                op: BinaryAssignOp::LogicAssign(logic_op.clone()),
                                lhs: analyzed_left,
                                rhs: Box::new(analyzed_right),
                            },
                        )
                    }
                },
                ParsedExpressionKind::FunctionCall { expr, args } => {
                    let new_len = output.len() - args.len();
                    let analyzed_args = output.split_off(new_len);
                    let arg_types = analyzed_args
                        .iter()
                        .map(|x| x.ty.clone())
                        .collect::<Vec<_>>();

                    determine_function_call(context, expr, arg_types, &location, analyzed_args)?
                }
                ParsedExpressionKind::Sizeof(ty) => {
                    let resolved_type = context
                        .types
                        .map_generic_parsed_type(&ty.value, context.generic_params)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Sizeof type '{}' not found at {}.",
                                ty.value,
                                ty.location
                            )
                        })?;
                    (
                        AnalyzedTypeId::Integer(4),
                        AnalyzedExpressionKind::Sizeof(resolved_type),
                    )
                }
                ParsedExpressionKind::Tuple(elements) => {
                    let analyzed_elements = output.split_off(output.len() - elements.len());
                    let element_types = analyzed_elements
                        .iter()
                        .map(|x| x.ty.clone())
                        .collect::<Vec<_>>();
                    (
                        context.types.get_tuple_type(&element_types),
                        AnalyzedExpressionKind::StructInstance {
                            fields: analyzed_elements
                                .into_iter()
                                .enumerate()
                                .map(|(i, x)| (format!("item{}", i), x))
                                .collect(),
                        },
                    )
                }
            };
            output.push(AnalyzedExpression {
                kind: analyzed,
                ty,
                location,
            });
        }
    }

    Ok(output.pop().unwrap())
}

fn assert_break_return_type(
    break_type: Option<&AnalyzedTypeId>,
    expr: &AnalyzedExpression,
) -> AnalyzerResult<Option<AnalyzedTypeId>> {
    match &expr.kind {
        AnalyzedExpressionKind::Block { expressions, .. } => {
            let mut actual_break_type = break_type.cloned();
            for expr in expressions.iter() {
                actual_break_type = assert_break_return_type(actual_break_type.as_ref(), expr)?;
            }
            Ok(actual_break_type)
        }
        AnalyzedExpressionKind::Return(maybe_expr) => {
            let mut actual_break_type = break_type.cloned();
            if let Some(expr) = maybe_expr {
                actual_break_type = assert_break_return_type(actual_break_type.as_ref(), expr)?;
            }
            Ok(actual_break_type)
        }
        AnalyzedExpressionKind::Continue => Ok(break_type.cloned()),
        AnalyzedExpressionKind::Break(maybe_expr) => {
            let expr_type = maybe_expr
                .as_ref()
                .map_or(AnalyzedTypeId::Unit, |e| e.ty.clone());
            if break_type.is_some_and(|x| *x != expr_type) {
                Err(anyhow::anyhow!(
                    "Break type '{}' does not match loop return type '{}' at {}.",
                    expr_type,
                    break_type.unwrap(),
                    expr.location
                ))
            } else {
                Ok(Some(expr_type))
            }
        }
        AnalyzedExpressionKind::If {
            condition,
            then_block,
            else_expr,
        } => {
            let mut new_break_type = assert_break_return_type(break_type, condition)?;
            new_break_type = assert_break_return_type(new_break_type.as_ref(), then_block)?;
            if let Some(else_expr) = else_expr {
                new_break_type = assert_break_return_type(new_break_type.as_ref(), else_expr)?;
            }
            Ok(new_break_type)
        }
        AnalyzedExpressionKind::Loop {
            init, else_expr, ..
        } => {
            let mut actual_break_type = break_type.cloned();
            if let Some(init) = init {
                actual_break_type = assert_break_return_type(actual_break_type.as_ref(), init)?;
            }
            if let Some(else_expr) = else_expr {
                actual_break_type =
                    assert_break_return_type(actual_break_type.as_ref(), else_expr)?;
            }
            Ok(actual_break_type)
        }
        AnalyzedExpressionKind::Declaration { value, .. } => {
            assert_break_return_type(break_type, value)
        }
        AnalyzedExpressionKind::ValueOfAssignable(assignable) => {
            assert_break_return_type_assignable(break_type, assignable)
        }
        AnalyzedExpressionKind::StructInstance { fields } => {
            let mut actual_break_type = break_type.cloned();
            for (_, field) in fields {
                actual_break_type = assert_break_return_type(actual_break_type.as_ref(), field)?;
            }
            Ok(actual_break_type)
        }
        AnalyzedExpressionKind::Literal(_) => Ok(break_type.cloned()),
        AnalyzedExpressionKind::ConstantPointer(_) => Ok(break_type.cloned()),
        AnalyzedExpressionKind::Unary { expr, .. } => assert_break_return_type(break_type, expr),
        AnalyzedExpressionKind::Binary { left, right, .. } => {
            let actual_break_type = assert_break_return_type(break_type, left)?;
            assert_break_return_type(actual_break_type.as_ref(), right)
        }
        AnalyzedExpressionKind::Assign { lhs, rhs, .. } => {
            let actual_break_type = assert_break_return_type_assignable(break_type, lhs)?;
            assert_break_return_type(actual_break_type.as_ref(), rhs)
        }
        AnalyzedExpressionKind::Borrow { expr } => {
            assert_break_return_type_assignable(break_type, expr)
        }
        AnalyzedExpressionKind::FunctionCall { args, call_type } => {
            let mut actual_break_type = match call_type {
                AnalyzedFunctionCallType::Function(_) => break_type.cloned(),
                AnalyzedFunctionCallType::FunctionPointer(inner) => {
                    assert_break_return_type(break_type, inner)?
                }
            };
            for arg in args {
                actual_break_type = assert_break_return_type(actual_break_type.as_ref(), arg)?;
            }
            Ok(actual_break_type)
        }
        AnalyzedExpressionKind::FieldAccess { expr, .. } => {
            assert_break_return_type(break_type, expr)
        }
        AnalyzedExpressionKind::Increment(expr, _) => {
            assert_break_return_type_assignable(break_type, expr)
        }
        AnalyzedExpressionKind::Decrement(expr, _) => {
            assert_break_return_type_assignable(break_type, expr)
        }
        AnalyzedExpressionKind::Sizeof(_) => Ok(break_type.cloned()),
        AnalyzedExpressionKind::FunctionPointer(_) => Ok(break_type.cloned()),
    }
}

fn assert_break_return_type_assignable(
    break_type: Option<&AnalyzedTypeId>,
    expr: &AssignableExpression,
) -> AnalyzerResult<Option<AnalyzedTypeId>> {
    match &expr.kind {
        AssignableExpressionKind::LocalVariable(_) => Ok(break_type.cloned()),
        AssignableExpressionKind::Dereference(expr) => assert_break_return_type(break_type, expr),
        AssignableExpressionKind::FieldAccess(expr, _) => {
            assert_break_return_type_assignable(break_type, expr)
        }
        AssignableExpressionKind::PointerFieldAccess(expr, _, _) => {
            assert_break_return_type(break_type, expr)
        }
        AssignableExpressionKind::ArrayIndex(arr, index) => {
            let actual = assert_break_return_type(break_type, arr)?;
            assert_break_return_type(actual.as_ref(), index)
        }
    }
}

pub fn resolve_generic_type(
    ty: &AnalyzedTypeId,
    generic_params: &GenericParams,
    generic_args: &Vec<AnalyzedTypeId>,
) -> AnalyzedTypeId {
    match ty {
        AnalyzedTypeId::GenericType(generic_id) => {
            generic_params.resolve(generic_id, generic_args).unwrap()
        }
        AnalyzedTypeId::Pointer(inner) => AnalyzedTypeId::Pointer(Box::new(resolve_generic_type(
            inner,
            generic_params,
            generic_args,
        ))),
        AnalyzedTypeId::StructType(struct_ref) => {
            let mut resolved_generic_args = Vec::new();
            for arg in struct_ref.generic_args.iter() {
                resolved_generic_args.push(resolve_generic_type(arg, generic_params, generic_args));
            }
            AnalyzedTypeId::StructType(StructRef {
                id: struct_ref.id.clone(),
                generic_args: resolved_generic_args,
            })
        }
        AnalyzedTypeId::FunctionType(return_type, params) => {
            let mut resolved_params = Vec::new();
            for param in params.iter() {
                resolved_params.push(resolve_generic_type(param, generic_params, generic_args));
            }
            AnalyzedTypeId::FunctionType(
                Box::new(resolve_generic_type(
                    return_type,
                    generic_params,
                    generic_args,
                )),
                resolved_params,
            )
        }
        _ => ty.clone(),
    }
}

pub fn check_matches_try_find_generic_args(
    arg_ty: &AnalyzedTypeId,
    param_ty: &AnalyzedTypeId,
    generic_arg_map: &mut HashMap<GenericId, AnalyzedTypeId>,
    location: &Location,
) -> AnalyzerResult<()> {
    match (arg_ty, param_ty) {
        (ty, AnalyzedTypeId::GenericType(generic_id)) => {
            if generic_arg_map
                .insert(generic_id.clone(), ty.clone())
                .is_some_and(|x| x != *ty)
            {
                return Err(anyhow::anyhow!(
                    "Ambiguous generic argument '{}' at {}",
                    generic_id,
                    location
                ));
            }
            Ok(())
        }
        (AnalyzedTypeId::Pointer(inner), AnalyzedTypeId::Pointer(inner2)) => {
            check_matches_try_find_generic_args(inner, inner2, generic_arg_map, location)
        }
        (AnalyzedTypeId::StructType(struct_ref), AnalyzedTypeId::StructType(struct_ref2)) => {
            for (arg, arg2) in struct_ref
                .generic_args
                .iter()
                .zip(struct_ref2.generic_args.iter())
            {
                check_matches_try_find_generic_args(arg, arg2, generic_arg_map, location)?
            }
            Ok(())
        }
        (
            AnalyzedTypeId::FunctionType(return_type, params),
            AnalyzedTypeId::FunctionType(return_type2, params2),
        ) => {
            for (param, param2) in params.iter().zip(params2.iter()) {
                check_matches_try_find_generic_args(param, param2, generic_arg_map, location)?;
            }
            check_matches_try_find_generic_args(
                return_type,
                return_type2,
                generic_arg_map,
                location,
            )
        }
        (ty, ty2) => {
            if ty == ty2 {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    "Type '{}' does not match expected type '{}' at {}",
                    ty,
                    ty2,
                    location
                ))
            }
        }
    }
}

fn analyze_generic_args(
    context: &mut AnalyzerContext,
    generic_args: &Vec<ParsedType>,
) -> AnalyzerResult<Vec<AnalyzedTypeId>> {
    generic_args
        .iter()
        .map(|arg| {
            context
                .types
                .map_generic_parsed_type(&arg.value, context.generic_params)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Generic argument '{}' not found at {}.",
                        arg.value,
                        arg.location
                    )
                })
        })
        .collect()
}

fn determine_variable_expression(
    context: &mut AnalyzerContext,
    var_name: &ParsedGenericId,
    location: &Location,
    type_hint: Option<AnalyzedTypeId>,
) -> AnalyzerResult<(AnalyzedTypeId, AnalyzedExpressionKind)> {
    if var_name.id.is_module_local && var_name.generic_args.is_empty() {
        let local_var = context.local_variables.get(&var_name.id.item_id.item_name);
        if let Some(local_var) = local_var {
            return Ok((
                local_var.ty.clone(),
                AnalyzedExpressionKind::ValueOfAssignable(AssignableExpression {
                    kind: AssignableExpressionKind::LocalVariable(
                        var_name.id.item_id.item_name.clone(),
                    ),
                    ty: local_var.ty.clone(),
                }),
            ));
        }
    }

    let generic_args = &var_name.generic_args;
    let analyzed_generic_args = analyze_generic_args(context, &generic_args)?;

    if let Some(AnalyzedTypeId::FunctionType(return_type, params)) = type_hint {
        let res = context.functions.map_function_id(
            &var_name.id,
            params,
            analyzed_generic_args,
            &location,
        );

        if let Ok(function_id) = res {
            return Ok((
                AnalyzedTypeId::FunctionType(return_type.clone(), function_id.arg_types.clone()),
                AnalyzedExpressionKind::FunctionPointer(function_id),
            ));
        } else if !var_name.generic_args.is_empty() {
            res?;
        }
    } else {
        let res = context.functions.function_id_from_scope_id(
            &var_name.id,
            analyzed_generic_args,
            &location,
        );

        if let Ok(function_id) = res {
            let header = context.functions.get_header(&function_id.id);
            let return_type = header.return_type.clone();
            let actual_return_type = resolve_generic_type(
                &return_type,
                &header.generic_params,
                &function_id.generic_args,
            );
            return Ok((
                AnalyzedTypeId::FunctionType(
                    Box::new(actual_return_type),
                    function_id.arg_types.clone(),
                ),
                AnalyzedExpressionKind::FunctionPointer(function_id),
            ));
        } else if !var_name.generic_args.is_empty() {
            res?;
        }
    }

    if !var_name.generic_args.is_empty() {
        return Err(anyhow::anyhow!(
            "Function '{}' not found at {}.",
            var_name,
            location
        ));
    }

    if !var_name.id.is_module_local {
        let enum_type = context
            .types
            .get_enum_from_variant(&var_name.id.item_id, &location.file.as_ref().unwrap().id);
        if let Some(enum_type) = enum_type {
            let val = enum_type
                .get_variant_value(&var_name.id.item_id.item_name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Enum variant '{}' not found at {}.",
                        var_name.id.item_id.item_name,
                        location
                    )
                })?;
            Ok((
                AnalyzedTypeId::EnumType(enum_type.id.clone()),
                AnalyzedExpressionKind::Literal(AnalyzedLiteral::Integer(val)),
            ))
        } else {
            Err(anyhow::anyhow!(
                "Expected module local identifier at {}.",
                location
            ))
        }
    } else {
        Err(anyhow::anyhow!(
            "Variable or function '{}' not found or ambiguous at {}.",
            var_name,
            location
        ))
    }
}

fn determine_function_call(
    context: &mut AnalyzerContext,
    expr: &ParsedExpression,
    arg_types: Vec<AnalyzedTypeId>,
    location: &Location,
    analyzed_args: Vec<AnalyzedExpression>,
) -> AnalyzerResult<(AnalyzedTypeId, AnalyzedExpressionKind)> {
    if let Ok(analyzed_expr) = analyze_expression(context, expr) {
        if let AnalyzedExpressionKind::FunctionPointer(_) = analyzed_expr.kind {
            //if the callee expression resolves to a single function pointer, we can just call it directly
        } else {
            if let AnalyzedTypeId::FunctionType(return_type, params) = analyzed_expr.ty.clone() {
                if params == arg_types {
                    return Ok((
                        *return_type,
                        AnalyzedExpressionKind::FunctionCall {
                            call_type: AnalyzedFunctionCallType::FunctionPointer(Box::new(
                                analyzed_expr,
                            )),
                            args: analyzed_args,
                        },
                    ));
                }
            }
        }
    }

    if let ParsedExpressionKind::Variable(id) = &expr.value {
        let generic_args = &id.generic_args;
        let analyzed_generic_args = analyze_generic_args(context, &generic_args)?;
        let function_ref_res = context.functions.map_function_id(
            &id.id,
            arg_types.clone(),
            analyzed_generic_args,
            &location,
        );

        let function_ref = if generic_args.len() == 0 && function_ref_res.is_err() {
            context
                .functions
                .map_function_id_guess_generics(&id.id, arg_types, &location)?
        } else {
            function_ref_res?
        };

        let function_header = context.functions.get_header(&function_ref.id);

        let return_type = function_header.return_type.clone();
        let actual_return_type = resolve_generic_type(
            &return_type,
            &function_header.generic_params,
            &function_ref.generic_args,
        );

        Ok((
            actual_return_type,
            AnalyzedExpressionKind::FunctionCall {
                call_type: AnalyzedFunctionCallType::Function(function_ref),
                args: analyzed_args,
            },
        ))
    } else {
        Err(anyhow::anyhow!(
            "Expected callable expression at {}.",
            location
        ))
    }
}
