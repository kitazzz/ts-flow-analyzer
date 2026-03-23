use crate::model::SymbolKind;
use oxc_ast::ast::*;

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
            FunctionNode::Function(f) => f.body.as_ref()?.statements.first().map(|s| s.span()),
            FunctionNode::Arrow(a) => a.body.statements.first().map(|s| s.span()),
            FunctionNode::Class(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct FieldInitializer<'a> {
    pub field_name: String,
    pub value: &'a Expression<'a>,
    pub span: oxc_span::Span,
}

#[derive(Debug)]
pub struct CollectedFunction<'a> {
    pub symbol_name: String,
    pub symbol_kind: SymbolKind,
    pub class_name: Option<String>,
    pub member_name: Option<String>,
    pub node: FunctionNode<'a>,
    pub start_line: u32,
    pub parent_class: Option<String>,
    pub is_abstract: bool,
    pub field_initializers: Vec<FieldInitializer<'a>>,
    pub has_implicit_super: bool,
}

pub fn collect_functions<'a>(program: &'a Program<'a>, source: &str) -> Vec<CollectedFunction<'a>> {
    let mut results = Vec::new();
    collect_from_statements(&program.body, source, &mut results);
    results
}

fn span_line(source: &str, start: u32) -> u32 {
    source[..start as usize]
        .bytes()
        .filter(|b| *b == b'\n')
        .count() as u32
        + 1
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
                    parent_class: None,
                    is_abstract: false,
                    field_initializers: Vec::new(),
                    has_implicit_super: false,
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
                                parent_class: None,
                                is_abstract: false,
                                field_initializers: Vec::new(),
                                has_implicit_super: false,
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
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
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
                        parent_class: None,
                        is_abstract: false,
                        field_initializers: Vec::new(),
                        has_implicit_super: false,
                    });
                }
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                collect_from_class(class, source, results);
            }
            _ => {}
        },
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
                        parent_class: None,
                        is_abstract: false,
                        field_initializers: Vec::new(),
                        has_implicit_super: false,
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
                        parent_class: None,
                        is_abstract: false,
                        field_initializers: Vec::new(),
                        has_implicit_super: false,
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
    let class_name = class
        .id
        .as_ref()
        .map(|id| id.name.to_string())
        .unwrap_or_default();

    let parent_class = class.super_class.as_ref().and_then(|expr| {
        if let Expression::Identifier(ident) = expr {
            Some(ident.name.to_string())
        } else {
            None
        }
    });

    if !class_name.is_empty() {
        let line = span_line(source, class.span.start);
        results.push(CollectedFunction {
            symbol_name: class_name.clone(),
            symbol_kind: SymbolKind::Class,
            class_name: Some(class_name.clone()),
            member_name: None,
            node: FunctionNode::Class(class),
            start_line: line,
            parent_class: parent_class.clone(),
            is_abstract: false,
            field_initializers: Vec::new(),
            has_implicit_super: false,
        });
    }

    // Pass 1: Collect field initializers from PropertyDefinition
    let mut field_inits: Vec<FieldInitializer<'a>> = Vec::new();
    for element in &class.body.body {
        if let ClassElement::PropertyDefinition(prop) = element {
            if prop.r#static || prop.declare {
                continue;
            }
            if let Some(ref value) = prop.value {
                let field_name = match &prop.key {
                    PropertyKey::StaticIdentifier(id) => id.name.to_string(),
                    PropertyKey::PrivateIdentifier(id) => format!("#{}", id.name),
                    _ => continue,
                };
                field_inits.push(FieldInitializer {
                    field_name,
                    value,
                    span: prop.span,
                });
            }
        }
    }

    // Pass 2: Collect methods, attaching field_inits to constructor
    let mut has_constructor = false;
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
                let is_ctor = method.kind.is_constructor();
                if is_ctor {
                    has_constructor = true;
                }
                let is_abstract = method.value.body.is_none();
                let line = span_line(source, method.span.start);
                results.push(CollectedFunction {
                    symbol_name,
                    symbol_kind: SymbolKind::Method,
                    class_name: if !class_name.is_empty() {
                        Some(class_name.clone())
                    } else {
                        None
                    },
                    member_name: Some(method_name),
                    node: FunctionNode::Function(&method.value),
                    start_line: line,
                    parent_class: parent_class.clone(),
                    is_abstract,
                    field_initializers: if is_ctor {
                        std::mem::take(&mut field_inits)
                    } else {
                        Vec::new()
                    },
                    has_implicit_super: false,
                });
            }
            _ => {}
        }
    }

    // Synthetic constructor: no explicit constructor but has field initializers
    if !has_constructor && !field_inits.is_empty() && !class_name.is_empty() {
        let symbol_name = format!("{}#constructor", class_name);
        let line = span_line(source, class.span.start);
        let is_derived = parent_class.is_some();
        results.push(CollectedFunction {
            symbol_name,
            symbol_kind: SymbolKind::Method,
            class_name: Some(class_name.clone()),
            member_name: Some("constructor".to_string()),
            node: FunctionNode::Class(class),
            start_line: line,
            parent_class: parent_class.clone(),
            is_abstract: false,
            field_initializers: std::mem::take(&mut field_inits),
            has_implicit_super: is_derived,
        });
    }
}
