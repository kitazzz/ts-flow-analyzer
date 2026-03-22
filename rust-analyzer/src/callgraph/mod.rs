pub mod classify;
pub mod collect_calls;
pub mod collect_imports;
pub mod dot;
pub mod model;
pub mod resolve;

use oxc_ast::ast::{Program, Statement};

use crate::ast::collect_functions::{CollectedFunction, FunctionNode};

use classify::classify_call;
use collect_calls::collect_calls;
use collect_imports::collect_imports;
use model::{CallCategory, CallEdge, CallGraphData, CallGraphNode};
use resolve::resolve_call;

fn get_statements<'a>(node: &'a FunctionNode<'a>) -> Option<&'a [Statement<'a>]> {
    match node {
        FunctionNode::Function(f) => f.body.as_ref().map(|b| b.statements.as_slice()),
        FunctionNode::Arrow(arrow) => Some(arrow.body.statements.as_slice()),
        FunctionNode::Class(_) => None,
    }
}

pub fn build_call_graph<'a>(
    collected: &[CollectedFunction<'a>],
    program: &Program<'a>,
    source: &str,
    include_builtins: bool,
) -> CallGraphData {
    let imports = collect_imports(program);
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
            let resolved = resolve_call(&call, func.class_name.as_deref(), collected, &imports);
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
            edges.push(CallEdge {
                caller: func.symbol_name.clone(),
                callee: resolved.as_ref().and_then(|r| r.as_internal_name()),
                kind: call.kind,
                target_name: call.target_name,
                receiver: call.receiver,
                line: call.line,
                import_source: resolved.as_ref().and_then(|r| r.as_import_source()),
                call_category: category,
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
        })
        .collect();

    CallGraphData {
        nodes,
        edges,
        imports,
    }
}
