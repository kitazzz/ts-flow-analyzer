use oxc_span::{GetSpan, Span};
use super::context::CfgContext;

/// Check if a basic block (identified by the span of the statement it contains)
/// has been marked unreachable by the CFG builder.
/// Returns Some(true) if unreachable, Some(false) if reachable, None if span not found.
pub fn is_span_unreachable(ctx: &CfgContext<'_>, stmt_span: Span) -> Option<bool> {
    let cfg = ctx.semantic.cfg()?;
    let nodes = ctx.semantic.nodes();
    // Collect matching (node_id, span) pairs first to avoid borrow conflicts
    let matched: Vec<_> = nodes
        .iter_enumerated()
        .filter(|(_, ast_node)| ast_node.span() == stmt_span)
        .map(|(node_id, _)| node_id)
        .collect();

    for node_id in matched {
        let block_id = nodes.cfg_id(node_id);
        let block = cfg.basic_block(block_id);
        return Some(block.is_unreachable());
    }
    None
}
