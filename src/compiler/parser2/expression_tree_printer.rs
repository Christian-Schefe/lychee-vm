use std::fmt::Display;
use crate::compiler::parser2::parsed_expression::{ParsedExpression, ParsedExpressionKind, ParsedFunction, ParsedLiteral, ParsedProgram, ParsedStructDefinition};

struct Line(usize, String);

impl Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let indent = "  ".repeat(self.0);
        write!(f, "{}{}", indent, self.1)
    }
}

struct Printer {
    lines: Vec<Line>,
    indent: usize,
}

impl Printer {
    pub fn new() -> Self {
        Printer {
            lines: Vec::new(),
            indent: 0,
        }
    }
    pub fn indent(&mut self) {
        self.indent += 1;
    }
    pub fn dedent(&mut self) {
        self.indent -= 1;
    }
    pub fn add_line(&mut self, line: String) {
        self.lines.push(Line(self.indent, line));
    }
    pub fn build(&self) -> String {
        self.lines.iter().map(|line| line.to_string()).collect::<Vec<String>>().join("\n")
    }
}

pub fn print_program(expr: &ParsedProgram) {
    let mut printer = Printer::new();
    for struct_def in &expr.struct_definitions {
        print_struct_definition(&mut printer, struct_def);
    }
    for function in &expr.functions {
        print_function(&mut printer, function);
    }
    println!("{}", printer.build());
}

fn print_struct_definition(printer: &mut Printer, struct_def: &ParsedStructDefinition) {
    printer.add_line(format!("struct {} {{", struct_def.struct_name));
    printer.indent();
    for (field_type, field_name) in &struct_def.fields {
        printer.add_line(format!("{}: {:?},", field_name, field_type.value));
    }
    printer.dedent();
    printer.add_line("}".to_string());
}

fn print_function(printer: &mut Printer, function: &ParsedFunction) {
    printer.add_line(format!("fn {}(", function.function_name));
    printer.indent();
    for (param_type, param_name) in &function.args {
        printer.add_line(format!("{}: {:?}", param_name, param_type.value));
    }
    printer.dedent();
    printer.add_line(")".to_string());
    print_expression(printer, &function.body);
}

fn print_expression(printer: &mut Printer, expr: &ParsedExpression) {
    match &expr.value {
        ParsedExpressionKind::Variable(name) => {
            printer.add_line(format!("Var({})", name));
        }
        ParsedExpressionKind::Continue => {
            printer.add_line("Continue".to_string());
        }
        ParsedExpressionKind::Break(maybe_expr) => {
            printer.add_line("Break".to_string());
            if let Some(expr) = maybe_expr {
                printer.indent();
                print_expression(printer, expr);
                printer.dedent();
            }
        }
        ParsedExpressionKind::Unary { expr, op } => {
            printer.add_line(format!("Unary({:?})", op));
            printer.indent();
            print_expression(printer, expr);
            printer.dedent();
        }
        ParsedExpressionKind::Binary { left, right, op } => {
            printer.add_line(format!("Binary({:?})", op));
            printer.indent();
            print_expression(printer, left);
            print_expression(printer, right);
            printer.dedent();
        }
        ParsedExpressionKind::Literal(lit) => {
            print_literal(printer, lit);
        }
        ParsedExpressionKind::Block(exprs) => {
            printer.add_line("{".to_string());
            printer.indent();
            for expr in exprs {
                print_expression(printer, expr);
            }
            printer.dedent();
            printer.add_line("}".to_string());
        }
        ParsedExpressionKind::Return(maybe_expr) => {
            printer.add_line("Return".to_string());
            if let Some(expr) = maybe_expr {
                printer.indent();
                print_expression(printer, expr);
                printer.dedent();
            }
        }
        ParsedExpressionKind::While { condition, loop_body, else_expr: false_block } => {
            printer.add_line("While".to_string());
            printer.indent();
            print_expression(printer, condition);
            print_expression(printer, loop_body);
            if let Some(false_block) = false_block {
                print_expression(printer, false_block);
            }
            printer.dedent();
        }
        ParsedExpressionKind::Declaration { var_type, var_name, value } => {
            printer.add_line(format!("Declaration({:?}, {})", var_type.value, var_name));
            printer.indent();
            print_expression(printer, value);
            printer.dedent();
        }
        ParsedExpressionKind::If { condition, then_block, else_expr: else_block } => {
            printer.add_line("If".to_string());
            printer.indent();
            print_expression(printer, condition);
            print_expression(printer, then_block);
            if let Some(else_block) = else_block {
                printer.add_line("Else".to_string());
                print_expression(printer, else_block);
            }
            printer.dedent();
        }
        ParsedExpressionKind::FunctionCall { function_name, args } => {
            printer.add_line(format!("FunctionCall({})", function_name));
            printer.indent();
            for arg in args {
                print_expression(printer, arg);
            }
            printer.dedent();
        }
    }
}

fn print_literal(printer: &mut Printer, lit: &ParsedLiteral) {
    match lit {
        ParsedLiteral::Integer(value) => {
            printer.add_line(format!("Int({})", value));
        }
        ParsedLiteral::Unit => {
            printer.add_line(format!("Unit"));
        }
        ParsedLiteral::String(value) => {
            printer.add_line(format!("String({})", value));
        }
        ParsedLiteral::Char(value) => {
            printer.add_line(format!("Char({})", value));
        }
        ParsedLiteral::Bool(value) => {
            printer.add_line(format!("Bool({})", value));
        }
        ParsedLiteral::Struct(struct_type, fields) => {
            printer.add_line(format!("Struct({:?})", struct_type.value));
            printer.indent();
            for (field_name, field_value) in fields {
                printer.add_line(format!("{}: ", field_name));
                print_expression(printer, field_value);
            }
            printer.dedent();
        }
    }
}