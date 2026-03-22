use std::collections::{BTreeSet, VecDeque};

use oxc_cfg::graph::visit::EdgeRef;
use oxc_cfg::EdgeType;
use oxc_semantic::dot::DebugDot;
use oxc_span::GetSpan;

use crate::ast::collect_functions::FunctionNode;

use super::context::CfgContext;

/// Render a function-local CFG subgraph as DOT.
///
/// The subgraph starts from the first statement in the function body when available,
/// falls back to the function node's CFG block otherwise, and does not cross
/// `NewFunction` edges into nested functions.
pub fn render_cfg_dot(ctx: &CfgContext<'_>, node: &FunctionNode<'_>) -> Option<String> {
    let cfg = ctx.semantic.cfg()?;

    let entry = if let Some(span) = node.first_stmt_span() {
        find_block_by_span(ctx, span).or_else(|| find_block_by_span(ctx, node.span()))?
    } else {
        find_block_by_span(ctx, node.span())?
    };

    let graph = cfg.graph();
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([entry]);

    while let Some(block) = queue.pop_front() {
        if !visited.insert(block) {
            continue;
        }

        for edge in graph.edges(block) {
            if matches!(edge.weight(), EdgeType::NewFunction) {
                continue;
            }
            queue.push_back(edge.target());
        }
    }

    if visited.is_empty() {
        return None;
    }

    let debug_ctx = ctx.semantic.nodes().into();
    let mut dot = String::from("digraph {\n");

    for block in &visited {
        let Some(basic_block_id) = graph.node_weight(*block) else {
            continue;
        };
        let basic_block = cfg.basic_block(*block);
        let label = basic_block
            .debug_dot(debug_ctx)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\\n");

        dot.push_str(&format!(
            "    {} [ label=\"{}\"",
            basic_block_id,
            escape_dot(&label)
        ));
        if basic_block.is_unreachable() {
            dot.push_str(", style=\"dotted\"");
        }
        dot.push_str("]\n");
    }

    for from in &visited {
        for edge in graph.edges(*from) {
            let to = edge.target();
            if !visited.contains(&to) {
                continue;
            }

            let Some(from_id) = graph.node_weight(*from) else {
                continue;
            };
            let Some(to_id) = graph.node_weight(to) else {
                continue;
            };

            dot.push_str(&format!(
                "    {} -> {} [ label=\"{}\"",
                from_id,
                to_id,
                escape_dot(&format!("{:?}", edge.weight()))
            ));

            if matches!(edge.weight(), EdgeType::Unreachable)
                || cfg.basic_block(*from).is_unreachable()
            {
                dot.push_str(", style=\"dotted\"");
            }

            match edge.weight() {
                EdgeType::Error(_) => dot.push_str(", color=\"red\""),
                EdgeType::Backedge => dot.push_str(", color=\"grey\""),
                EdgeType::Jump => dot.push_str(", color=\"green\""),
                _ => {}
            }

            dot.push_str("]\n");
        }
    }

    dot.push_str("}\n");
    Some(dot)
}

fn find_block_by_span(ctx: &CfgContext<'_>, span: oxc_span::Span) -> Option<oxc_cfg::BlockNodeId> {
    let nodes = ctx.semantic.nodes();
    nodes.iter_enumerated().find_map(|(node_id, ast_node)| {
        (ast_node.kind().span() == span).then(|| nodes.cfg_id(node_id))
    })
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
