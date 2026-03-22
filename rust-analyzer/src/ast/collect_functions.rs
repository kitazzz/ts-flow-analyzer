use oxc_ast::ast::*;
use crate::model::SymbolKind;

/// A reference to a function node we want to analyze.
#[derive(Debug)]
pub enum FunctionNode<'a> {
    Function(&'a Function<'a>),
    Arrow(&'a ArrowFunctionExpression<'a>),
    Class(&'a Class<'a>),
}

impl<'a> FunctionNode<'a> {
    pub fn span(&self) -> oxc_span::Span {
        match self {
            FunctionNode::Function(f) => f.span,
            FunctionNode::Arrow(a) => a.span,
            FunctionNode::Class(c) => c.span,
        }
    }

    pub fn first_stmt_span(&self) -> Option<oxc_span::Span> {
        use oxc_span::GetSpan;
        match self {
            FunctionNode::Function(f) => {
                f.body.as_ref()?.statements.first().map(|s| s.span())
            }
            FunctionNode::Arrow(a) => {
                a.body.statements.first().map(|s| s.span())
            }
            FunctionNode::Class(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct CollectedFunction<'a> {
    pub symbol_name: String,
    pub symbol_kind: SymbolKind,
    pub class_name: Option<String>,
    pub member_name: Option<String>,
    pub node: FunctionNode<'a>,
    pub start_line: u32,
}

pub fn collect_functions<'a>(program: &'a Program<'a>, source: &str) -> Vec<CollectedFunction<'a>> {
    let mut results = Vec::new();
    collect_from_statements(&program.body, source, &mut results);
    results
}

fn span_line(source: &str, start: u32) -> u32 {
    source[..start as usize].bytes().filter(|b| *b == b'\n').count() as u32 + 1
}

fn collect_from_statements<'a>(
    stmts: &'a [Statement<'a>],
    source: &str,
    results: &mut Vec<CollectedFunction<'a>>,
) {
    for stmt in stmts {
        collect_from_statement(stmt, source, results);
    }
}

fn collect_from_statement<'a>(
    stmt: &'a Statement<'a>,
    source: &str,
    results: &mut Vec<CollectedFunction<'a>>,
) {
    match stmt {
        Statement::FunctionDeclaration(func) => {
            if let Some(id) = &func.id {
                let name = id.name.to_string();
                let line = span_line(source, func.span.start);
                results.push(CollectedFunction {
                    symbol_name: name.clone(),
                    symbol_kind: SymbolKind::Function,
                    class_name: None,
                    member_name: Some(name),
                    node: FunctionNode::Function(func),
                    start_line: line,
                });
            }
        }
        Statement::VariableDeclaration(var_decl) => {
            collect_from_var_decl(var_decl, source, results);
        }
        Statement::ClassDeclaration(class) => {
            collect_from_class(class, source, results);
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(decl) = &export.declaration {
                match decl {
                    Declaration::FunctionDeclaration(func) => {
                        if let Some(id) = &func.id {
                            let name = id.name.to_string();
                            let line = span_line(source, func.span.start);
                            results.push(CollectedFunction {
                                symbol_name: name.clone(),
                                symbol_kind: SymbolKind::Function,
                                class_name: None,
                                member_name: Some(name),
                                node: FunctionNode::Function(func),
                                start_line: line,
                            });
                        }
                    }
                    Declaration::VariableDeclaration(var_decl) => {
                        collect_from_var_decl(var_decl, source, results);
                    }
                    Declaration::ClassDeclaration(class) => {
                        collect_from_class(class, source, results);
                    }
                    _ => {}
                }
            }
        }
        Statement::ExportDefaultDeclaration(export) => {
            match &export.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                    if let Some(id) = &func.id {
                        let name = id.name.to_string();
                        let line = span_line(source, func.span.start);
                        results.push(CollectedFunction {
                            symbol_name: name.clone(),
                            symbol_kind: SymbolKind::Function,
                            class_name: None,
                            member_name: Some(name),
                            node: FunctionNode::Function(func),
                            start_line: line,
                        });
                    }
                }
                ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                    collect_from_class(class, source, results);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn collect_from_var_decl<'a>(
    var_decl: &'a VariableDeclaration<'a>,
    source: &str,
    results: &mut Vec<CollectedFunction<'a>>,
) {
    for declarator in &var_decl.declarations {
        if let Some(init) = &declarator.init {
            let name = match &declarator.id {
                BindingPattern::BindingIdentifier(id) => id.name.to_string(),
                _ => continue,
            };
            match init {
                Expression::FunctionExpression(func) => {
                    let line = span_line(source, var_decl.span.start);
                    results.push(CollectedFunction {
                        symbol_name: name.clone(),
                        symbol_kind: SymbolKind::VariableFunction,
                        class_name: None,
                        member_name: Some(name),
                        node: FunctionNode::Function(func),
                        start_line: line,
                    });
                }
                Expression::ArrowFunctionExpression(arrow) => {
                    let line = span_line(source, var_decl.span.start);
                    results.push(CollectedFunction {
                        symbol_name: name.clone(),
                        symbol_kind: SymbolKind::VariableFunction,
                        class_name: None,
                        member_name: Some(name),
                        node: FunctionNode::Arrow(arrow),
                        start_line: line,
                    });
                }
                _ => {}
            }
        }
    }
}

fn collect_from_class<'a>(
    class: &'a Class<'a>,
    source: &str,
    results: &mut Vec<CollectedFunction<'a>>,
) {
    let class_name = class.id.as_ref().map(|id| id.name.to_string()).unwrap_or_default();
    if !class_name.is_empty() {
        let line = span_line(source, class.span.start);
        results.push(CollectedFunction {
            symbol_name: class_name.clone(),
            symbol_kind: SymbolKind::Class,
            class_name: Some(class_name.clone()),
            member_name: None,
            node: FunctionNode::Class(class),
            start_line: line,
        });
    }

    // Collect methods
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(method) => {
                let method_name = match &method.key {
                    PropertyKey::StaticIdentifier(id) => id.name.to_string(),
                    PropertyKey::PrivateIdentifier(id) => format!("#{}", id.name),
                    _ => continue,
                };
                let symbol_name = if !class_name.is_empty() {
                    format!("{}#{}", class_name, method_name)
                } else {
                    method_name.clone()
                };
                let line = span_line(source, method.span.start);
                results.push(CollectedFunction {
                    symbol_name: symbol_name.clone(),
                    symbol_kind: SymbolKind::Method,
                    class_name: if !class_name.is_empty() { Some(class_name.clone()) } else { None },
                    member_name: Some(method_name),
                    node: FunctionNode::Function(&method.value),
                    start_line: line,
                });
            }
            _ => {}
        }
    }
}
