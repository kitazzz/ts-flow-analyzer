pub mod classify;
pub mod collect_calls;
pub mod collect_imports;
pub mod dot;
pub mod model;
pub mod resolve;

use std::collections::HashMap;

use oxc_ast::ast::{Program, Statement};

use crate::ast::collect_functions::{CollectedFunction, FunctionNode};
use crate::model::SymbolKind;

use classify::classify_call;
use collect_calls::collect_calls;
use collect_imports::collect_imports;
use model::{CallCategory, CallEdge, CallGraphData, CallGraphNode, CallKind, ResolvedTarget};
use resolve::resolve_call;

fn get_statements<'a>(node: &'a FunctionNode<'a>) -> Option<&'a [Statement<'a>]> {
    match node {
        FunctionNode::Function(f) => f.body.as_ref().map(|b| b.statements.as_slice()),
        FunctionNode::Arrow(arrow) => Some(arrow.body.statements.as_slice()),
        FunctionNode::Class(_) => None,
    }
}

/// Build a map from class name to its parent class name (from `extends`).
fn build_class_hierarchy(collected: &[CollectedFunction<'_>]) -> HashMap<String, Option<String>> {
    collected
        .iter()
        .filter(|f| matches!(f.symbol_kind, SymbolKind::Class))
        .filter_map(|f| {
            f.class_name
                .as_ref()
                .map(|name| (name.clone(), f.parent_class.clone()))
        })
        .collect()
}

/// Check if `cls` is a descendant of `ancestor` in the hierarchy.
fn is_descendant_of(
    cls: &str,
    ancestor: &str,
    hierarchy: &HashMap<String, Option<String>>,
) -> bool {
    let mut current = hierarchy.get(cls).and_then(|p| p.clone());
    while let Some(parent) = current {
        if parent == ancestor {
            return true;
        }
        current = hierarchy.get(&parent).and_then(|p| p.clone());
    }
    false
}

/// Find concrete overrides of `method_name` in all subclasses of `class_name`.
///
/// We intentionally do NOT skip subclasses that override the caller method,
/// because `override caller() { super.caller() }` still executes the base
/// caller body. Static analysis cannot determine whether super is called,
/// so the sound approach is to include all concrete overrides.
fn find_concrete_overrides(
    class_name: &str,
    method_name: &str,
    hierarchy: &HashMap<String, Option<String>>,
    symbols: &[CollectedFunction<'_>],
) -> Vec<String> {
    let mut overrides = Vec::new();
    for (cls, _) in hierarchy {
        if !is_descendant_of(cls, class_name, hierarchy) {
            continue;
        }
        let target = format!("{}#{}", cls, method_name);
        if symbols
            .iter()
            .any(|s| s.symbol_name == target && !s.is_abstract)
        {
            overrides.push(target);
        }
    }
    overrides
}

pub fn build_call_graph<'a>(
    collected: &[CollectedFunction<'a>],
    program: &Program<'a>,
    source: &str,
    include_builtins: bool,
) -> CallGraphData {
    let imports = collect_imports(program);
    let hierarchy = build_class_hierarchy(collected);
    let mut edges = Vec::new();

    for func in collected {
        if matches!(func.node, FunctionNode::Class(_)) {
            continue;
        }
        let stmts = match get_statements(&func.node) {
            Some(s) => s,
            None => continue,
        };
        let calls = collect_calls(stmts, source);
        for call in calls {
            let resolved =
                resolve_call(&call, func.class_name.as_deref(), collected, &imports, &hierarchy);
            let category = classify_call(
                &call.kind,
                &call.target_name,
                call.receiver.as_deref(),
                resolved.as_ref(),
            );
            // Filter builtins unless explicitly requested
            if !include_builtins && category == CallCategory::Builtin {
                continue;
            }

            // Override dispatch for ThisMethod calls
            if matches!(call.kind, CallKind::ThisMethod) {
                if let Some(class_name) = &func.class_name {
                    let overrides = find_concrete_overrides(
                        class_name,
                        &call.target_name,
                        &hierarchy,
                        collected,
                    );
                    let has_concrete_override = !overrides.is_empty();

                    // If the base callee is abstract and there are concrete overrides,
                    // suppress the abstract base edge
                    let skip_base = has_concrete_override
                        && resolved
                            .as_ref()
                            .and_then(|r| r.as_internal_name())
                            .map(|name| {
                                collected
                                    .iter()
                                    .any(|f| f.symbol_name == name && f.is_abstract)
                            })
                            .unwrap_or(false);

                    if !skip_base {
                        edges.push(CallEdge {
                            caller: func.symbol_name.clone(),
                            callee: resolved.as_ref().and_then(|r| r.as_internal_name()),
                            kind: CallKind::ThisMethod,
                            target_name: call.target_name.clone(),
                            receiver: call.receiver.clone(),
                            line: call.line,
                            span_start: Some(call.span_start),
                            span_end: Some(call.span_end),
                            import_source: resolved
                                .as_ref()
                                .and_then(|r| r.as_import_source()),
                            call_category: category,
                            imported_name: resolved.as_ref().and_then(|r| match r {
                                ResolvedTarget::Import(entry) => {
                                    Some(entry.imported_name.clone())
                                }
                                _ => None,
                            }),
                        });
                    }

                    // Add override dispatch edges
                    for override_target in overrides {
                        if resolved
                            .as_ref()
                            .and_then(|r| r.as_internal_name())
                            .as_deref()
                            == Some(override_target.as_str())
                        {
                            continue;
                        }
                        edges.push(CallEdge {
                            caller: func.symbol_name.clone(),
                            callee: Some(override_target),
                            kind: CallKind::ThisMethod,
                            target_name: call.target_name.clone(),
                            receiver: call.receiver.clone(),
                            line: call.line,
                            span_start: Some(call.span_start),
                            span_end: Some(call.span_end),
                            import_source: None,
                            call_category: CallCategory::Domain,
                            imported_name: None,
                        });
                    }

                    continue; // Already handled, skip the default edge push below
                }
            }

            edges.push(CallEdge {
                caller: func.symbol_name.clone(),
                callee: resolved.as_ref().and_then(|r| r.as_internal_name()),
                kind: call.kind,
                target_name: call.target_name,
                receiver: call.receiver,
                line: call.line,
                span_start: Some(call.span_start),
                span_end: Some(call.span_end),
                import_source: resolved.as_ref().and_then(|r| r.as_import_source()),
                call_category: category,
                imported_name: resolved.as_ref().and_then(|r| match r {
                    ResolvedTarget::Import(entry) => Some(entry.imported_name.clone()),
                    _ => None,
                }),
            });
        }
    }

    let nodes = collected
        .iter()
        .filter(|f| !matches!(f.node, FunctionNode::Class(_)))
        .map(|f| CallGraphNode {
            symbol_name: f.symbol_name.clone(),
            kind: f.symbol_kind.to_string(),
            class_name: f.class_name.clone(),
            start_line: f.start_line,
            parent_class: f.parent_class.clone(),
        })
        .collect();

    CallGraphData {
        nodes,
        edges,
        imports,
    }
}
