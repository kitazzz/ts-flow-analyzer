use oxc_span::GetSpan;

use super::context::CfgContext;

/// Map an AST span to the CFG block that contains it.
///
/// Iterates all semantic nodes looking for an exact span match, then returns
/// the corresponding CFG block ID. Used by CFG DOT rendering, `from_cfg` graph
/// construction, and ICFG call-site mapping.
pub(crate) fn find_block_by_span(
    ctx: &CfgContext<'_>,
    span: oxc_span::Span,
) -> Option<oxc_cfg::BlockNodeId> {
    let nodes = ctx.semantic.nodes();
    nodes.iter_enumerated().find_map(|(node_id, ast_node)| {
        (ast_node.kind().span() == span).then(|| nodes.cfg_id(node_id))
    })
}
