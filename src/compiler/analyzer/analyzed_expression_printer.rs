use crate::compiler::analyzer::analyzed_expression::{
    AnalyzedExpression, AnalyzedExpressionKind, AnalyzedFunctionCallType, AnalyzedProgram,
    AssignableExpression, AssignableExpressionKind,
};
use crate::compiler::merger::merged_expression::ResolvedFunctionHeader;
use crate::compiler::parser::expression_tree_printer::Printer;
use std::path::PathBuf;

pub fn print_program(program: &AnalyzedProgram, output_path: &PathBuf) {
    let mut printer = Printer::new();
    for (id, function_body) in program.function_bodies.iter() {
        let header = program.resolved_functions.get_header(id);
        print_function_header(&mut printer, header);
        print_expression(&mut printer, function_body);
    }
    let output = printer.build();
    std::fs::write(output_path, output).unwrap();
}

fn print_function_header(printer: &mut Printer, header: &ResolvedFunctionHeader) {
    printer.add_line(format!(
        "Function<{:?}>({})",
        header.generic_params, header.id
    ));
    printer.indent();
    for (param_name, param_type) in &header.parameter_types {
        printer.add_line(format!("{}: {}", param_name, param_type));
    }
    printer.dedent();
    printer.add_line(format!("-> {}", header.return_type));
}

fn print_expression(printer: &mut Printer, expr: &AnalyzedExpression) {
    printer.add_line(format!("Expr type: {}", expr.ty));
    match &expr.kind {
        AnalyzedExpressionKind::Block {
            expressions,
            returns_value,
        } => {
            printer.add_line(format!("Block (returns: {}) {{", returns_value));
            printer.indent();
            for expr in expressions {
                print_expression(printer, expr);
            }
            printer.dedent();
            printer.add_line("}".to_string());
        }
        AnalyzedExpressionKind::Return(inner) => {
            printer.add_line("Return {".to_string());
            printer.indent();
            if let Some(inner) = inner {
                print_expression(printer, inner);
            }
            printer.dedent();
            printer.add_line("}".to_string());
        }
        AnalyzedExpressionKind::Continue => {
            printer.add_line("Continue".to_string());
        }
        AnalyzedExpressionKind::Break(inner) => {
            printer.add_line("Break {".to_string());
            printer.indent();
            if let Some(inner) = inner {
                print_expression(printer, inner);
            }
            printer.dedent();
            printer.add_line("}".to_string());
        }
        AnalyzedExpressionKind::If {
            condition,
            then_block,
            else_expr,
        } => {
            printer.add_line("If {".to_string());
            printer.indent();
            print_expression(printer, condition);
            print_expression(printer, then_block);
            if let Some(else_expr) = else_expr {
                print_expression(printer, else_expr);
            }
            printer.dedent();
            printer.add_line("}".to_string());
        }
        AnalyzedExpressionKind::Loop {
            init,
            condition,
            step,
            loop_body,
            else_expr,
        } => {
            printer.add_line("Loop {".to_string());
            printer.indent();
            if let Some(init) = init {
                print_expression(printer, init);
            }
            if let Some(condition) = condition {
                print_expression(printer, condition);
            }
            if let Some(step) = step {
                print_expression(printer, step);
            }
            print_expression(printer, loop_body);
            if let Some(else_expr) = else_expr {
                print_expression(printer, else_expr);
            }
            printer.dedent();
            printer.add_line("}".to_string());
        }
        AnalyzedExpressionKind::Declaration { var_name, value } => {
            printer.add_line(format!("Declaration({})", var_name));
            printer.indent();
            print_expression(printer, value);
            printer.dedent();
        }
        AnalyzedExpressionKind::ValueOfAssignable(inner) => {
            printer.add_line("ValueOfAssignable".to_string());
            printer.indent();
            print_assignable_expression(printer, inner);
            printer.dedent();
        }
        AnalyzedExpressionKind::StructInstance { fields } => {
            printer.add_line("Struct".to_string());
            printer.indent();
            for (field_name, field_value) in fields {
                printer.add_line(format!("{}: ", field_name));
                print_expression(printer, field_value);
            }
            printer.dedent();
        }
        AnalyzedExpressionKind::Literal(lit) => {
            printer.add_line(format!("Literal({:?})", lit));
        }
        AnalyzedExpressionKind::ConstantPointer(constant) => {
            printer.add_line(format!("ConstantPointer({:?})", constant));
        }
        AnalyzedExpressionKind::Unary { op, expr } => {
            printer.add_line(format!("Unary({:?})", op));
            printer.indent();
            print_expression(printer, expr);
            printer.dedent();
        }
        AnalyzedExpressionKind::Binary { op, left, right } => {
            printer.add_line(format!("Binary({:?})", op));
            printer.indent();
            print_expression(printer, left);
            print_expression(printer, right);
            printer.dedent();
        }
        AnalyzedExpressionKind::Assign { op, lhs, rhs } => {
            printer.add_line(format!("Assign({:?})", op));
            printer.indent();
            print_assignable_expression(printer, lhs);
            print_expression(printer, rhs);
            printer.dedent();
        }
        AnalyzedExpressionKind::Borrow { expr } => {
            printer.add_line("Borrow".to_string());
            printer.indent();
            print_assignable_expression(printer, expr);
            printer.dedent();
        }
        AnalyzedExpressionKind::FunctionCall { call_type, args } => {
            printer.add_line("FunctionCall".to_string());
            printer.indent();
            match call_type {
                AnalyzedFunctionCallType::Function(function) => {
                    printer.add_line(format!("Function({})", function));
                }
                AnalyzedFunctionCallType::FunctionPointer(inner) => {
                    print_expression(printer, inner);
                }
            }
            for arg in args {
                print_expression(printer, arg);
            }
            printer.dedent();
        }
        AnalyzedExpressionKind::FieldAccess { field_name, expr } => {
            printer.add_line(format!("FieldAccess({})", field_name));
            printer.indent();
            print_expression(printer, expr);
            printer.dedent();
        }
        AnalyzedExpressionKind::Increment(inner, is_post) => {
            printer.add_line(format!("Increment({})", is_post));
            printer.indent();
            print_assignable_expression(printer, inner);
            printer.dedent();
        }
        AnalyzedExpressionKind::Decrement(inner, is_post) => {
            printer.add_line(format!("Decrement({})", is_post));
            printer.indent();
            print_assignable_expression(printer, inner);
            printer.dedent();
        }
        AnalyzedExpressionKind::Sizeof(ty) => {
            printer.add_line(format!("Sizeof({:?})", ty));
        }
        AnalyzedExpressionKind::FunctionPointer(function) => {
            printer.add_line(format!("FunctionPointer({})", function));
        }
    }
}

fn print_assignable_expression(printer: &mut Printer, expr: &AssignableExpression) {
    match &expr.kind {
        AssignableExpressionKind::LocalVariable(var) => {
            printer.add_line(format!("LocalVariable({})", var));
        }
        AssignableExpressionKind::Dereference(inner) => {
            printer.add_line("Dereference".to_string());
            printer.indent();
            print_expression(printer, inner);
            printer.dedent();
        }
        AssignableExpressionKind::FieldAccess(inner, field) => {
            printer.add_line(format!("FieldAccess({})", field));
            printer.indent();
            print_assignable_expression(printer, inner);
            printer.dedent();
        }
        AssignableExpressionKind::PointerFieldAccess(inner, field, indirections) => {
            printer.add_line(format!("PointerFieldAccess({}, {})", field, indirections));
            printer.indent();
            print_expression(printer, inner);
            printer.dedent();
        }
        AssignableExpressionKind::ArrayIndex(array, index) => {
            printer.add_line("ArrayIndex".to_string());
            printer.indent();
            print_expression(printer, array);
            print_expression(printer, index);
            printer.dedent();
        }
    }
}
