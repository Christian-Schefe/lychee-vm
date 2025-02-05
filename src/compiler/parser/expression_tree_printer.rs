use crate::compiler::parser::parsed_expression::{
    ParsedExpression, ParsedExpressionKind, ParsedFunction, ParsedFunctionSignature, ParsedModule,
    ParsedProgram, ParsedStructDefinition, ParsedTraitDefinition, ParsedTraitImplementation,
};
use std::fmt::Display;
use std::path::PathBuf;

struct Line(usize, String);

impl Display for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let indent = "  ".repeat(self.0);
        write!(f, "{}{}", indent, self.1)
    }
}

pub struct Printer {
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
        self.lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<String>>()
            .join("\n")
    }
}

pub fn print_program(program: &ParsedProgram, output_path: &PathBuf) {
    let mut printer = Printer::new();
    for (_, module) in &program.module_tree {
        print_module(&mut printer, module);
    }
    let output = printer.build();
    std::fs::write(output_path, output).unwrap();
}

pub fn print_module(printer: &mut Printer, expr: &ParsedModule) {
    printer.add_line(format!("Module({})", expr.module_path.get_identifier()));
    for import in &expr.imports {
        if let Some(objects) = &import.value.imported_objects {
            for object in objects {
                printer.add_line(format!(
                    "Import({}, {})",
                    object,
                    import.value.module_id.get_identifier()
                ));
            }
        } else {
            printer.add_line(format!(
                "Import(*) from {}",
                import.value.module_id.get_identifier()
            ));
        }
    }
    for struct_def in &expr.struct_definitions {
        print_struct_definition(printer, &struct_def.value);
    }
    for alias in &expr.type_aliases {
        printer.add_line(format!(
            "TypeAlias({}, {})",
            alias.value.alias, alias.value.aliased_type.value
        ));
    }
    for enum_def in &expr.enums {
        printer.add_line(format!("Enum({})", enum_def.value.enum_name));
        printer.indent();
        for (variant_name, variant_type) in &enum_def.value.variants {
            printer.add_line(format!("{}: {:?}", variant_name, variant_type));
        }
        printer.dedent();
    }
    for trait_def in &expr.trait_definitions {
        print_trait_definition(printer, &trait_def.value);
    }
    for trait_impl in &expr.trait_implementations {
        print_trait_impl(printer, &trait_impl.value);
    }
    for function in &expr.functions {
        print_function(printer, &function.value);
    }
}

fn print_trait_definition(printer: &mut Printer, trait_def: &ParsedTraitDefinition) {
    printer.add_line(format!(
        "trait {}<{:?}> {{",
        trait_def.trait_name, trait_def.generics
    ));
    printer.indent();
    for function in &trait_def.functions {
        print_function_signature(printer, &function.value);
    }
    printer.dedent();
    printer.add_line("}".to_string());
}

fn print_trait_impl(printer: &mut Printer, trait_impl: &ParsedTraitImplementation) {
    printer.add_line(format!(
        "impl {} for {} {{",
        trait_impl.trait_id, trait_impl.for_type.value
    ));
    printer.indent();
    for function in &trait_impl.functions {
        print_function(printer, &function.value);
    }
    printer.dedent();
    printer.add_line("}".to_string());
}

fn print_struct_definition(printer: &mut Printer, struct_def: &ParsedStructDefinition) {
    printer.add_line(format!("struct {} {{", struct_def.struct_name));
    printer.indent();
    for (field_name, field_type) in &struct_def.fields {
        printer.add_line(format!("{}: {},", field_name, field_type.value));
    }
    printer.dedent();
    printer.add_line("}".to_string());
}

fn print_function_signature(printer: &mut Printer, signature: &ParsedFunctionSignature) {
    printer.add_line(format!(
        "fn {}<{:?}>(",
        signature.function_name, signature.generic_params
    ));
    printer.indent();
    for (param_type, param_name) in &signature.params {
        printer.add_line(format!("{}: {}", param_name, param_type.value));
    }
    printer.dedent();
    printer.add_line(format!(") -> {}", signature.return_type.value));
}

fn print_function(printer: &mut Printer, function: &ParsedFunction) {
    print_function_signature(printer, &function.signature);
    print_expression(printer, &function.body);
}

fn print_expression(printer: &mut Printer, expr: &ParsedExpression) {
    match &expr.value {
        ParsedExpressionKind::Sizeof(ty) => {
            printer.add_line(format!("Sizeof({:?})", ty));
        }
        ParsedExpressionKind::StructInstance {
            struct_type,
            fields,
        } => {
            printer.add_line(format!("StructInstance({:?})", struct_type));
            printer.indent();
            for (field_name, field_value) in fields {
                printer.add_line(format!("{}: ", field_name));
                print_expression(printer, field_value);
            }
            printer.dedent();
        }
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
            printer.add_line(format!("Literal({:?})", lit));
        }
        ParsedExpressionKind::Block {
            expressions,
            returns_value: _,
        } => {
            printer.add_line("{".to_string());
            printer.indent();
            for expr in expressions {
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
        ParsedExpressionKind::Loop {
            init,
            condition,
            step,
            loop_body,
            else_expr,
        } => {
            printer.add_line("Loop".to_string());
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
        }
        ParsedExpressionKind::Declaration {
            var_type,
            var_name,
            value,
        } => {
            printer.add_line(format!(
                "Declaration({}, {})",
                var_type
                    .as_ref()
                    .map_or("var".to_string(), |x| format!("{:?}", x.value)),
                var_name
            ));
            printer.indent();
            print_expression(printer, value);
            printer.dedent();
        }
        ParsedExpressionKind::If {
            condition,
            then_block,
            else_expr: else_block,
        } => {
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
        ParsedExpressionKind::FunctionCall { expr, args } => {
            printer.add_line(format!("FunctionCall"));
            printer.indent();
            print_expression(printer, expr);
            for arg in args {
                print_expression(printer, arg);
            }
            printer.dedent();
        }
        ParsedExpressionKind::Tuple(elements) => {
            printer.add_line("Tuple".to_string());
            printer.indent();
            for expr in elements {
                print_expression(printer, expr);
            }
            printer.dedent();
        }
    }
}
