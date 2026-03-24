use std::collections::{BTreeSet, HashMap, VecDeque};

use oxc_cfg::graph::visit::EdgeRef;
use oxc_cfg::EdgeType;
use oxc_semantic::dot::DebugDot;
use oxc_span::GetSpan;

use crate::ast::collect_functions::FunctionNode;
use crate::control_flow::context::CfgContext;
use crate::control_flow::util::find_block_by_span;

use super::builder::GraphBuilder;
use super::graph::*;

/// Result of building CFG graph nodes for a single function.
/// Exposed so ICFG can connect its edges to existing CfgBlock graph nodes.
pub struct CfgBuildResult {
    pub block_to_node: HashMap<oxc_cfg::BlockNodeId, NodeId>,
    pub entry_block: oxc_cfg::BlockNodeId,
    pub reachable_blocks: BTreeSet<oxc_cfg::BlockNodeId>,
}

pub fn cfg_to_graph(
    ctx: &CfgContext<'_>,
    node: &FunctionNode<'_>,
    parent_id: NodeId,
    builder: &mut GraphBuilder,
) -> Option<CfgBuildResult> {
    let cfg = ctx.semantic.cfg()?;

    // Entry resolution: same 2-stage fallback as render_cfg_dot (control_flow/dot.rs:20-24)
    let entry = if let Some(span) = node.first_stmt_span() {
        find_block_by_span(ctx, span).or_else(|| find_block_by_span(ctx, node.span()))
    } else {
        find_block_by_span(ctx, node.span())
    };
    let entry = entry?;

    let graph = cfg.graph();

    // BFS to collect reachable blocks (excluding NewFunction edges)
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
    let mut block_to_node: HashMap<oxc_cfg::BlockNodeId, NodeId> = HashMap::new();

    // Create CfgBlock nodes
    for &block in &visited {
        let Some(_basic_block_id) = graph.node_weight(block) else {
            continue;
        };
        let basic_block = cfg.basic_block(block);
        let full_label = basic_block
            .debug_dot(debug_ctx)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\\n");

        let label = if full_label.len() > 80 {
            format!("{}...", &full_label[..77])
        } else {
            full_label.clone()
        };

        // Try to get line from first instruction's AST node span
        let line = basic_block.instructions().first().and_then(|inst| {
            let node_id = inst.node_id?;
            let ast_node = ctx.semantic.nodes().get_node(node_id);
            let span = ast_node.kind().span();
            if span.start > 0 {
                let source = ctx.semantic.source_text();
                Some(source[..span.start as usize].bytes().filter(|b| *b == b'\n').count() as u32 + 1)
            } else {
                None
            }
        });

        let id = builder.add_node(
            NodeKind::CfgBlock,
            label,
            None,
            SourceLoc {
                line,
                span_start: None,
                span_end: None,
            },
        );

        // Add properties
        if full_label.len() > 80 {
            builder.nodes.last_mut().unwrap().properties.insert(
                "fullLabel".to_string(),
                serde_json::Value::String(full_label),
            );
        }
        if basic_block.is_unreachable() {
            builder.nodes.last_mut().unwrap().properties.insert(
                "unreachable".to_string(),
                serde_json::Value::Bool(true),
            );
        }

        builder.add_edge(parent_id, id, EdgeKind::Contains, None);
        block_to_node.insert(block, id);
    }

    // Create Cfg edges
    for &from in &visited {
        for edge in graph.edges(from) {
            let to = edge.target();
            if !visited.contains(&to) {
                continue;
            }
            let Some(&from_id) = block_to_node.get(&from) else { continue };
            let Some(&to_id) = block_to_node.get(&to) else { continue };

            let cfg_type = match edge.weight() {
                EdgeType::Normal => CfgEdgeType::Normal,
                EdgeType::Backedge => CfgEdgeType::Backedge,
                EdgeType::Jump => CfgEdgeType::Jump,
                EdgeType::Unreachable => CfgEdgeType::Unreachable,
                EdgeType::Error(_) => CfgEdgeType::Error,
                EdgeType::NewFunction => continue,
                // Finalize, Join, and any future variants → Normal
                _ => CfgEdgeType::Normal,
            };

            builder.add_edge(
                from_id,
                to_id,
                EdgeKind::Cfg { cfg_type },
                None,
            );
        }
    }

    Some(CfgBuildResult {
        block_to_node,
        entry_block: entry,
        reachable_blocks: visited,
    })
}
