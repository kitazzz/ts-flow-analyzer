use std::collections::HashMap;

use crate::callgraph::collect_imports::collect_imports;
use crate::callgraph::model::ImportEntry;
use crate::config::ResolverFactoriesConfig;
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
    collect_functions_with_resolver_config(program, source, None)
}

pub fn collect_functions_with_resolver_config<'a>(
    program: &'a Program<'a>,
    source: &str,
    resolver_factories: Option<&ResolverFactoriesConfig>,
) -> Vec<CollectedFunction<'a>> {
    let mut results = Vec::new();
    let ctx = CollectContext {
        resolver_factories,
        imports_by_local_name: build_import_map(program),
    };
    collect_from_statements(&program.body, source, &ctx, &mut results);
    results
}

struct CollectContext<'cfg> {
    resolver_factories: Option<&'cfg ResolverFactoriesConfig>,
    imports_by_local_name: HashMap<String, Vec<ImportEntry>>,
}

fn build_import_map(program: &Program<'_>) -> HashMap<String, Vec<ImportEntry>> {
    let mut imports_by_local_name: HashMap<String, Vec<ImportEntry>> = HashMap::new();
    for entry in collect_imports(program) {
        imports_by_local_name
            .entry(entry.local_name.clone())
            .or_default()
            .push(entry);
    }
    imports_by_local_name
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
    ctx: &CollectContext<'_>,
    results: &mut Vec<CollectedFunction<'a>>,
) {
    for stmt in stmts {
        collect_from_statement(stmt, source, ctx, results);
    }
}

fn collect_from_statement<'a>(
    stmt: &'a Statement<'a>,
    source: &str,
    ctx: &CollectContext<'_>,
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
            collect_from_var_decl(var_decl, source, ctx, results);
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
                        collect_from_var_decl(var_decl, source, ctx, results);
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
            kind => {
                if let Some(expr) = kind.as_expression() {
                    if let Some(call) = unwrap_to_call_expr(expr) {
                        if let Some(extraction) = try_extract_factory_call(call, ctx) {
                            push_factory_function(extraction, "default", call.span.start, source, ctx, results);
                        }
                    }
                }
            }
        },
        _ => {}
    }
}

fn collect_from_var_decl<'a>(
    var_decl: &'a VariableDeclaration<'a>,
    source: &str,
    ctx: &CollectContext<'_>,
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
                init => {
                    if let Some(call) = unwrap_to_call_expr(init) {
                        if let Some(extraction) = try_extract_factory_call(call, ctx) {
                            push_factory_function(extraction, &name, var_decl.span.start, source, ctx, results);
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory call extraction (e.g. createResolver)
// ---------------------------------------------------------------------------

const FACTORY_FUNCTIONS: &[&str] = &["createResolver"];

struct FactoryCallExtraction<'a> {
    config_name: Option<String>,
    body_node: FunctionNode<'a>,
}

/// Unwrap TS type-assertion / parenthesized wrappers and return the inner
/// `CallExpression` if one is present. This covers patterns like:
///   createResolver({...}) as ResolverType
///   createResolver({...}) satisfies ResolverType
///   (createResolver({...}))
fn unwrap_to_call_expr<'a>(expr: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expr {
        Expression::CallExpression(call) => Some(call),
        Expression::TSAsExpression(e) => unwrap_to_call_expr(&e.expression),
        Expression::TSSatisfiesExpression(e) => unwrap_to_call_expr(&e.expression),
        Expression::TSNonNullExpression(e) => unwrap_to_call_expr(&e.expression),
        Expression::TSTypeAssertion(e) => unwrap_to_call_expr(&e.expression),
        Expression::ParenthesizedExpression(e) => unwrap_to_call_expr(&e.expression),
        _ => None,
    }
}

/// Unwrap TS type-assertion / parenthesized wrappers around a value expression
/// to reach the underlying function node.
fn unwrap_body_expr<'a>(expr: &'a Expression<'a>) -> Option<FunctionNode<'a>> {
    match expr {
        Expression::ArrowFunctionExpression(a) => Some(FunctionNode::Arrow(a)),
        Expression::FunctionExpression(f) => Some(FunctionNode::Function(f)),
        Expression::TSAsExpression(e) => unwrap_body_expr(&e.expression),
        Expression::TSSatisfiesExpression(e) => unwrap_body_expr(&e.expression),
        Expression::TSNonNullExpression(e) => unwrap_body_expr(&e.expression),
        Expression::TSTypeAssertion(e) => unwrap_body_expr(&e.expression),
        Expression::ParenthesizedExpression(e) => unwrap_body_expr(&e.expression),
        _ => None,
    }
}

/// For resolver bodies that are thin wrappers around a transaction callback,
/// unwrap to the inner callback so analysis passes see the real logic.
///
/// A body is "thin" when:
/// 1. The last statement is `return [await] someCall(..., callback)`
/// 2. ALL preceding statements are `VariableDeclaration` (infrastructure setup only)
///
/// If the prelude contains any control-flow or side-effect statements (if, throw,
/// expression statements, etc.), the body has real logic and is NOT unwrapped.
///
/// When unwrapped, the inner callback's params replace the outer's (e.g. `trx`
/// instead of `context`), and outer scope variables become captured free variables
/// in data-flow. The `start_line` stays at the declaration level, but `node.span()`
/// will point to the inner callback — this is an intentional trade-off.
fn unwrap_thin_wrapper<'a>(node: FunctionNode<'a>) -> FunctionNode<'a> {
    let stmts: &[Statement<'a>] = match &node {
        FunctionNode::Arrow(a) => &a.body.statements,
        FunctionNode::Function(f) => match &f.body {
            Some(b) => &b.statements,
            None => return node,
        },
        FunctionNode::Class(_) => return node,
    };

    if stmts.is_empty() {
        return node;
    }

    // All preceding statements (everything except the last) must be VariableDeclaration.
    // Any control flow (if, throw, for, …) or expression statements means the outer
    // body has real logic and should not be unwrapped.
    let (prelude, last) = stmts.split_at(stmts.len() - 1);
    for s in prelude {
        if !matches!(s, Statement::VariableDeclaration(_)) {
            return node;
        }
    }

    // Last statement must be ReturnStatement
    let ret_arg = match &last[0] {
        Statement::ReturnStatement(ret) => match &ret.argument {
            Some(arg) => arg,
            None => return node,
        },
        _ => return node,
    };

    // Unwrap optional await
    let call_expr = match ret_arg {
        Expression::AwaitExpression(aw) => &aw.argument,
        other => other,
    };

    // Must be a CallExpression (unwrap TS wrappers too)
    let call = match unwrap_to_call_expr(call_expr) {
        Some(c) => c,
        None => return node,
    };

    // Find the last Arrow/Function argument (callback is typically the last arg)
    call.arguments
        .iter()
        .rev()
        .find_map(|arg| match arg {
            Argument::SpreadElement(_) => None,
            other => unwrap_body_expr(other.to_expression()),
        })
        .unwrap_or(node)
}

fn try_extract_factory_call<'a>(
    call: &'a CallExpression<'a>,
    ctx: &CollectContext<'_>,
) -> Option<FactoryCallExtraction<'a>> {
    if !is_factory_call(call, ctx) {
        return None;
    }

    // First argument must be an object expression (not spread)
    let first_arg = call.arguments.first()?;
    let obj = match first_arg {
        Argument::ObjectExpression(obj) => obj,
        _ => return None,
    };

    let mut config_name: Option<String> = None;
    let mut body_node: Option<FunctionNode<'a>> = None;

    for prop in &obj.properties {
        let ObjectPropertyKind::ObjectProperty(p) = prop else {
            continue;
        };
        let key_name = match &p.key {
            PropertyKey::StaticIdentifier(id) => Some(&*id.name),
            PropertyKey::StringLiteral(s) => Some(&*s.value),
            _ => None,
        };
        match key_name {
            Some("name") => {
                if let Expression::StringLiteral(s) = &p.value {
                    config_name = Some(s.value.to_string());
                }
            }
            Some("body") => {
                body_node = unwrap_body_expr(&p.value);
            }
            _ => {}
        }
    }

    Some(FactoryCallExtraction {
        config_name,
        body_node: body_node?,
    })
}

fn is_factory_call(call: &CallExpression<'_>, ctx: &CollectContext<'_>) -> bool {
    let local_name = match &call.callee {
        Expression::Identifier(id) => id.name.as_str(),
        _ => return false,
    };

    if let Some(resolver_factories) = ctx.resolver_factories {
        if resolver_factories.has_presets() {
            return ctx
                .imports_by_local_name
                .get(local_name)
                .is_some_and(|entries| {
                    entries.iter().any(|entry| {
                        resolver_factories.matches_import(&entry.imported_name, &entry.source)
                    })
                });
        }
    }

    FACTORY_FUNCTIONS.contains(&local_name)
}

fn push_factory_function<'a>(
    extraction: FactoryCallExtraction<'a>,
    fallback_name: &str,
    decl_span_start: u32,
    source: &str,
    ctx: &CollectContext<'_>,
    results: &mut Vec<CollectedFunction<'a>>,
) {
    let name = extraction
        .config_name
        .unwrap_or_else(|| fallback_name.to_string());
    let line = span_line(source, decl_span_start);
    let body_node = if ctx
        .resolver_factories
        .is_some_and(|rf| rf.has_tailor_sdk())
    {
        unwrap_thin_wrapper(extraction.body_node)
    } else {
        extraction.body_node
    };
    results.push(CollectedFunction {
        symbol_name: name.clone(),
        symbol_kind: SymbolKind::Resolver,
        class_name: None,
        member_name: Some(name),
        node: body_node,
        start_line: line,
        parent_class: None,
        is_abstract: false,
        field_initializers: Vec::new(),
        has_implicit_super: false,
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolverFactoriesConfig;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser as OxcParser;
    use oxc_span::SourceType;

    fn collect_summary(source: &str) -> Vec<(String, SymbolKind, u32)> {
        collect_summary_with_config(source, None)
    }

    fn collect_summary_with_config(
        source: &str,
        resolver_factories: Option<&ResolverFactoriesConfig>,
    ) -> Vec<(String, SymbolKind, u32)> {
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, source, SourceType::ts()).parse();
        assert!(ret.errors.is_empty(), "parse errors: {:?}", ret.errors);
        let collected = collect_functions_with_resolver_config(&ret.program, source, resolver_factories);
        collected
            .iter()
            .map(|f| (f.symbol_name.clone(), f.symbol_kind.clone(), f.start_line))
            .collect()
    }

    #[test]
    fn export_default_create_resolver() {
        let src = r#"
export default createResolver({
  name: "approveOrder",
  body: async (context) => {
    if (!context.input.id) { throw new Error("missing id"); }
    return { id: context.input.id };
  },
});
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "approveOrder");
        assert!(matches!(result[0].1, SymbolKind::Resolver));
        assert_eq!(result[0].2, 1);
    }

    #[test]
    fn const_and_export_const_create_resolver() {
        let src = r#"
const getOrder = createResolver({
  name: "getOrder",
  body: async (ctx) => { return ctx; },
});
export const deleteOrder = createResolver({
  name: "deleteOrder",
  body: async (ctx) => { return ctx; },
});
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "getOrder");
        assert!(matches!(result[0].1, SymbolKind::Resolver));
        assert_eq!(result[0].2, 1);
        assert_eq!(result[1].0, "deleteOrder");
        assert!(matches!(result[1].1, SymbolKind::Resolver));
        assert_eq!(result[1].2, 5);
    }

    #[test]
    fn name_fallback_to_variable_name() {
        let src = r#"
const myResolver = createResolver({
  body: () => { return {}; },
});
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "myResolver");
        assert!(matches!(result[0].1, SymbolKind::Resolver));
    }

    #[test]
    fn name_fallback_to_default() {
        let src = r#"
export default createResolver({
  body: () => { return {}; },
});
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "default");
    }

    #[test]
    fn non_factory_call_ignored() {
        let src = r#"export default someOtherFactory({ body: () => {} });"#;
        let result = collect_summary(src);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn quoted_keys() {
        let src = r#"
export default createResolver({
  "name": "quotedResolver",
  "body": async (ctx) => { return ctx; },
});
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "quotedResolver");
        assert!(matches!(result[0].1, SymbolKind::Resolver));
    }

    #[test]
    fn function_expression_body() {
        let src = r#"
export default createResolver({
  name: "funcBody",
  body: function(ctx) { return ctx; },
});
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "funcBody");
        assert!(matches!(result[0].1, SymbolKind::Resolver));
    }

    #[test]
    fn body_with_parenthesized_wrapper() {
        let src = r#"
export default createResolver({
  name: "wrappedBody",
  body: ((async (ctx) => { return ctx; })),
});
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "wrappedBody");
    }

    #[test]
    fn call_site_satisfies_wrapper() {
        let src = r#"
export default createResolver({
  name: "satisfiesResolver",
  body: async (ctx) => { return ctx; },
}) satisfies any;
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "satisfiesResolver");
    }

    #[test]
    fn call_site_as_wrapper() {
        let src = r#"
const typed = createResolver({
  name: "asResolver",
  body: async (ctx) => { return ctx; },
}) as any;
"#
        .trim_start();
        let result = collect_summary(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "asResolver");
    }

    #[test]
    fn tailor_sdk_preset_matches_aliased_import() {
        let src = r#"
import { createResolver as cr } from "@tailor-platform/sdk";

export default cr({
  name: "aliasedResolver",
  body: async (ctx) => { return ctx; },
});
"#
        .trim_start();
        let resolver_factories = ResolverFactoriesConfig {
            presets: vec!["tailor-sdk".to_string()],
        };
        let result = collect_summary_with_config(src, Some(&resolver_factories));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "aliasedResolver");
        assert!(matches!(result[0].1, SymbolKind::Resolver));
    }

    #[test]
    fn tailor_sdk_preset_rejects_other_sources() {
        let src = r#"
import { createResolver } from "some-other-sdk";

export default createResolver({
  name: "wrongSource",
  body: async (ctx) => { return ctx; },
});
"#
        .trim_start();
        let resolver_factories = ResolverFactoriesConfig {
            presets: vec!["tailor-sdk".to_string()],
        };
        let result = collect_summary_with_config(src, Some(&resolver_factories));
        assert_eq!(result.len(), 0);
    }

    // --- thin-wrapper unwrap tests (tailor-sdk gated) ---

    fn tailor_sdk_config() -> ResolverFactoriesConfig {
        ResolverFactoriesConfig {
            presets: vec!["tailor-sdk".to_string()],
        }
    }

    #[test]
    fn resolver_transaction_unwrap() {
        let src = r#"
import { createResolver } from "@tailor-platform/sdk";

export default createResolver({
  name: "txResolver",
  body: async (context) => {
    const db = getDB("main-db");
    return db.transaction().execute(async (trx) => {
      const result = await doSomething(trx);
      if (!result.ok) { throw new Error("fail"); }
      return { id: result.id };
    });
  },
});
"#
        .trim_start();
        let cfg = tailor_sdk_config();
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, src, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        let collected = collect_functions_with_resolver_config(&ret.program, src, Some(&cfg));
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].symbol_name, "txResolver");
        // Node should be the INNER arrow (trx callback), not the outer (context) arrow
        match &collected[0].node {
            FunctionNode::Arrow(a) => {
                // Inner body: const result + if + return = 3 statements
                assert_eq!(a.body.statements.len(), 3);
            }
            _ => panic!("expected Arrow for inner callback"),
        }
    }

    #[test]
    fn resolver_transaction_unwrap_await() {
        let src = r#"
import { createResolver } from "@tailor-platform/sdk";

export default createResolver({
  name: "awaitTx",
  body: async (context) => {
    return await db.transaction().execute(async (trx) => {
      return { ok: true };
    });
  },
});
"#
        .trim_start();
        let cfg = tailor_sdk_config();
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, src, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        let collected = collect_functions_with_resolver_config(&ret.program, src, Some(&cfg));
        assert_eq!(collected.len(), 1);
        match &collected[0].node {
            FunctionNode::Arrow(a) => {
                // Inner body: return { ok: true }; = 1 statement
                assert_eq!(a.body.statements.len(), 1);
            }
            _ => panic!("expected Arrow"),
        }
    }

    #[test]
    fn resolver_non_thin_wrapper_preserved() {
        let src = r#"
import { createResolver } from "@tailor-platform/sdk";

export default createResolver({
  name: "directBody",
  body: async (ctx) => {
    if (!ctx.input.id) { throw new Error("missing"); }
    return { id: ctx.input.id };
  },
});
"#
        .trim_start();
        let cfg = tailor_sdk_config();
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, src, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        let collected = collect_functions_with_resolver_config(&ret.program, src, Some(&cfg));
        assert_eq!(collected.len(), 1);
        match &collected[0].node {
            FunctionNode::Arrow(a) => {
                // Original body: if + return = 2 statements (return has no callback)
                assert_eq!(a.body.statements.len(), 2);
            }
            _ => panic!("expected Arrow"),
        }
    }

    #[test]
    fn resolver_outer_logic_not_unwrapped() {
        // Critical negative test: if-statement in prelude blocks unwrap
        let src = r#"
import { createResolver } from "@tailor-platform/sdk";

export default createResolver({
  name: "outerLogic",
  body: async (context) => {
    if (!context.input.id) { throw new Error("missing"); }
    return db.transaction().execute(async (trx) => {
      return { id: trx.result };
    });
  },
});
"#
        .trim_start();
        let cfg = tailor_sdk_config();
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, src, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        let collected = collect_functions_with_resolver_config(&ret.program, src, Some(&cfg));
        assert_eq!(collected.len(), 1);
        match &collected[0].node {
            FunctionNode::Arrow(a) => {
                // Original body preserved: if + return = 2 statements
                // NOT unwrapped because prelude contains IfStatement
                assert_eq!(a.body.statements.len(), 2);
            }
            _ => panic!("expected Arrow"),
        }
    }

    #[test]
    fn resolver_prelude_domain_call_unwrapped() {
        // Spec: VariableDeclaration with domain call in prelude IS accepted
        // (the heuristic is VariableDeclaration-only, not "infrastructure-only")
        let src = r#"
import { createResolver } from "@tailor-platform/sdk";

export default createResolver({
  name: "domainPrelude",
  body: async (context) => {
    const flag = someDomainCheck(context.input);
    return db.execute(async (trx) => {
      if (!flag) { throw new Error("invalid"); }
      return { ok: true };
    });
  },
});
"#
        .trim_start();
        let cfg = tailor_sdk_config();
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, src, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        let collected = collect_functions_with_resolver_config(&ret.program, src, Some(&cfg));
        assert_eq!(collected.len(), 1);
        match &collected[0].node {
            FunctionNode::Arrow(a) => {
                // Inner callback: if + return = 2 statements
                // Unwrapped because prelude is only VariableDeclaration
                assert_eq!(a.body.statements.len(), 2);
            }
            _ => panic!("expected Arrow"),
        }
    }

    #[test]
    fn resolver_return_no_callback_preserved() {
        let src = r#"
import { createResolver } from "@tailor-platform/sdk";

export default createResolver({
  name: "noCallback",
  body: async (ctx) => {
    return await someService.process(ctx.input);
  },
});
"#
        .trim_start();
        let cfg = tailor_sdk_config();
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, src, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        let collected = collect_functions_with_resolver_config(&ret.program, src, Some(&cfg));
        assert_eq!(collected.len(), 1);
        match &collected[0].node {
            FunctionNode::Arrow(a) => {
                // No callback arg in the call → original body preserved (1 return stmt)
                assert_eq!(a.body.statements.len(), 1);
            }
            _ => panic!("expected Arrow"),
        }
    }

    #[test]
    fn resolver_no_preset_skips_unwrap() {
        // Without tailor-sdk preset, unwrap is NOT applied even for thin wrapper pattern
        let src = r#"
export default createResolver({
  name: "noPreset",
  body: async (context) => {
    const db = getDB("main-db");
    return db.execute(async (trx) => {
      return { ok: true };
    });
  },
});
"#
        .trim_start();
        // Use collect_functions (no config) — no preset
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, src, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        let collected = collect_functions(&ret.program, src);
        assert_eq!(collected.len(), 1);
        match &collected[0].node {
            FunctionNode::Arrow(a) => {
                // Original outer body: const db + return = 2 statements (NOT unwrapped)
                assert_eq!(a.body.statements.len(), 2);
            }
            _ => panic!("expected Arrow"),
        }
    }
}
