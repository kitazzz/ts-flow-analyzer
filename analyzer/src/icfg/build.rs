use std::collections::{BTreeSet, HashMap, VecDeque};

use oxc_cfg::graph::visit::EdgeRef;
use oxc_cfg::{BlockNodeId, EdgeType};
use oxc_span::Span;

use crate::callgraph::model::CallGraphData;
use crate::control_flow::context::CfgContext;
use crate::control_flow::util::find_block_by_span;
use crate::ir::from_cfg::CfgBuildResult;

use super::model::*;

/// Build the single-file ICFG report from existing CFG and call graph data.
///
/// Only resolved internal calls (where `CallEdge.callee` is `Some` and present
/// in `cfg_results`) produce interprocedural connections.
pub fn build_icfg(
    ctx: &CfgContext<'_>,
    call_graph: &CallGraphData,
    cfg_results: &HashMap<String, CfgBuildResult>,
) -> IcfgReport {
    // Phase 1: Build per-function entry/exit info
    let mut functions = HashMap::new();
    for (symbol_name, cfg_result) in cfg_results {
        let exit_blocks = find_exit_blocks(ctx, cfg_result);
        functions.insert(
            symbol_name.clone(),
            FunctionIcfgInfo {
                entry_block: cfg_result.entry_block,
                exit_blocks,
            },
        );
    }

    // Phase 2: Build interprocedural connections from resolved internal calls
    let mut connections = Vec::new();
    for edge in &call_graph.edges {
        // Only resolved internal calls (callee is the resolved symbol_name)
        let Some(callee_name) = &edge.callee else {
            continue;
        };
        // Both caller and callee must have CFG results
        let Some(caller_cfg) = cfg_results.get(&edge.caller) else {
            continue;
        };
        if !cfg_results.contains_key(callee_name) {
            continue;
        }
        // Need call expression span for block lookup
        let Some(span_start) = edge.span_start else {
            continue;
        };
        let Some(span_end) = edge.span_end else {
            continue;
        };

        // Find the call-site's CFG block in the caller
        let Some(call_block) = find_call_block(ctx, span_start, span_end, caller_cfg) else {
            continue;
        };

        // Find return-site blocks (flow-edge successors of call block in caller)
        let return_blocks = find_return_blocks(ctx, call_block, caller_cfg);

        connections.push(IcfgConnection {
            caller_symbol: edge.caller.clone(),
            callee_symbol: callee_name.clone(),
            call_site_block: call_block,
            return_site_blocks: return_blocks,
        });
    }

    IcfgReport {
        functions,
        connections,
    }
}

/// Whether an edge type represents normal control flow (not error/unreachable/new-function).
/// Matches `from_cfg.rs` catch-all: Finalize, Join, and future variants are flow edges.
fn is_flow_edge(edge_type: &EdgeType) -> bool {
    !matches!(
        edge_type,
        EdgeType::Error(_) | EdgeType::Unreachable | EdgeType::NewFunction
    )
}

/// Find exit blocks: blocks reachable via **normal flow edges only** (not Error/Unreachable/
/// NewFunction) that have no outgoing flow edges to other normal-reachable blocks.
///
/// This excludes error-sink blocks that are only reachable via Error edges, so that
/// `FunctionExit` represents normal function completion, not exceptional paths.
fn find_exit_blocks(ctx: &CfgContext<'_>, cfg_result: &CfgBuildResult) -> Vec<BlockNodeId> {
    let Some(cfg) = ctx.semantic.cfg() else {
        return vec![];
    };
    let graph = cfg.graph();

    // BFS from entry following only flow edges (excludes Error/Unreachable/NewFunction)
    let mut normal_reachable = BTreeSet::new();
    let mut queue = VecDeque::from([cfg_result.entry_block]);
    while let Some(block) = queue.pop_front() {
        if !normal_reachable.insert(block) {
            continue;
        }
        for edge in graph.edges(block) {
            if is_flow_edge(edge.weight()) {
                queue.push_back(edge.target());
            }
        }
    }

    // Exit blocks: normal-reachable blocks with no outgoing flow edges to other normal-reachable
    normal_reachable
        .iter()
        .filter(|&&block| {
            !graph.edges(block).any(|edge| {
                is_flow_edge(edge.weight())
                    && normal_reachable.contains(&edge.target())
            })
        })
        .copied()
        .collect()
}

/// Map a call expression span to its containing CFG block in the caller.
/// Returns `None` if the span doesn't match or the block isn't in the caller's CFG.
fn find_call_block(
    ctx: &CfgContext<'_>,
    span_start: u32,
    span_end: u32,
    caller_cfg: &CfgBuildResult,
) -> Option<BlockNodeId> {
    let span = Span::new(span_start, span_end);
    let block = find_block_by_span(ctx, span)?;
    // Verify the block belongs to this caller's reachable set
    caller_cfg
        .reachable_blocks
        .contains(&block)
        .then_some(block)
}

/// Find return-site blocks: flow-edge successors of the call-site block
/// within the caller's reachable set.
fn find_return_blocks(
    ctx: &CfgContext<'_>,
    call_block: BlockNodeId,
    caller_cfg: &CfgBuildResult,
) -> Vec<BlockNodeId> {
    let Some(cfg) = ctx.semantic.cfg() else {
        return vec![];
    };
    let graph = cfg.graph();
    graph
        .edges(call_block)
        .filter(|edge| is_flow_edge(edge.weight()))
        .map(|edge| edge.target())
        .filter(|target| caller_cfg.reachable_blocks.contains(target))
        .collect()
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    use crate::ast::collect_functions::{collect_functions, FunctionNode};
    use crate::callgraph::build_call_graph;
    use crate::control_flow::build_cfg_context;
    use crate::ir::from_cfg::cfg_to_graph;

    use super::*;

    /// Helper: parse TS source, collect functions, build call graph + CFG, build ICFG
    fn build_test_icfg(source: &str) -> IcfgReport {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path("test.ts").unwrap();
        let ret = Parser::new(&allocator, source, source_type).parse();
        let collected = collect_functions(&ret.program, source);
        let call_graph = build_call_graph(&collected, &ret.program, source, false);
        let cfg_ctx = build_cfg_context(&ret.program);

        // Build CFG graph nodes to get CfgBuildResults
        let mut builder = crate::ir::builder::GraphBuilder::new();
        let symbol_map = crate::ir::from_functions::functions_to_graph(&collected, &mut builder);
        let mut cfg_results = HashMap::new();
        for func in &collected {
            if matches!(func.node, FunctionNode::Class(_)) {
                continue;
            }
            if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                if let Some(result) = cfg_to_graph(&cfg_ctx, &func.node, parent_id, &mut builder) {
                    cfg_results.insert(func.symbol_name.clone(), result);
                }
            }
        }

        build_icfg(&cfg_ctx, &call_graph, &cfg_results)
    }

    #[test]
    fn icfg_simple_direct_call() {
        let report = build_test_icfg(
            r#"
            function caller() {
              const x = callee();
              return x;
            }
            function callee() {
              return 42;
            }
            "#,
        );
        assert_eq!(report.functions.len(), 2);
        assert!(report.functions.contains_key("caller"));
        assert!(report.functions.contains_key("callee"));
        assert_eq!(report.connections.len(), 1);
        assert_eq!(report.connections[0].caller_symbol, "caller");
        assert_eq!(report.connections[0].callee_symbol, "callee");
    }

    #[test]
    fn icfg_multiple_calls() {
        let report = build_test_icfg(
            r#"
            function main() {
              foo();
              bar();
              return;
            }
            function foo() { return 1; }
            function bar() { return 2; }
            "#,
        );
        assert_eq!(report.connections.len(), 2);
        let callee_names: Vec<&str> = report
            .connections
            .iter()
            .map(|c| c.callee_symbol.as_str())
            .collect();
        assert!(callee_names.contains(&"foo"));
        assert!(callee_names.contains(&"bar"));
    }

    #[test]
    fn icfg_recursive_call() {
        let report = build_test_icfg(
            r#"
            function fib(n: number): number {
              if (n <= 1) return n;
              return fib(n - 1) + fib(n - 2);
            }
            "#,
        );
        // Two recursive calls to self
        assert_eq!(report.connections.len(), 2);
        for conn in &report.connections {
            assert_eq!(conn.caller_symbol, "fib");
            assert_eq!(conn.callee_symbol, "fib");
        }
    }

    #[test]
    fn icfg_this_method_call() {
        let report = build_test_icfg(
            r#"
            class Service {
              process() {
                return this.validate();
              }
              validate() {
                return true;
              }
            }
            "#,
        );
        let method_connections: Vec<_> = report
            .connections
            .iter()
            .filter(|c| c.caller_symbol == "Service#process")
            .collect();
        assert_eq!(method_connections.len(), 1);
        assert_eq!(method_connections[0].callee_symbol, "Service#validate");
    }

    #[test]
    fn icfg_unresolved_excluded() {
        let report = build_test_icfg(
            r#"
            import { externalFn } from './other';
            function caller() {
              externalFn();
              unknownObj.method();
            }
            "#,
        );
        // No connections — externalFn is an import, unknownObj.method is unresolved
        assert_eq!(report.connections.len(), 0);
    }

    #[test]
    fn icfg_graph_ir_roundtrip() {
        use crate::ir::from_callgraph::callgraph_to_graph;
        use crate::ir::from_icfg::icfg_to_graph;
        use crate::ir::graph::{EdgeKind, IcfgEdgeType, NodeKind};

        // Use a pattern where the call is followed by a branch, ensuring the call
        // block has flow-edge successors (return-site blocks for ICFG).
        let source = r#"
            function caller() {
              const x = callee();
              if (x > 0) {
                return "positive";
              }
              return "negative";
            }
            function callee() {
              return 42;
            }
        "#;

        let allocator = Allocator::default();
        let source_type = SourceType::from_path("test.ts").unwrap();
        let ret = Parser::new(&allocator, source, source_type).parse();
        let collected = collect_functions(&ret.program, source);
        let call_graph = build_call_graph(&collected, &ret.program, source, false);
        let cfg_ctx = build_cfg_context(&ret.program);

        let mut builder = crate::ir::builder::GraphBuilder::new();
        let symbol_map = crate::ir::from_functions::functions_to_graph(&collected, &mut builder);
        callgraph_to_graph(&call_graph, &symbol_map, &mut builder);

        let mut cfg_results = HashMap::new();
        for func in &collected {
            if matches!(func.node, FunctionNode::Class(_)) {
                continue;
            }
            if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                if let Some(result) = cfg_to_graph(&cfg_ctx, &func.node, parent_id, &mut builder) {
                    cfg_results.insert(func.symbol_name.clone(), result);
                }
            }
        }

        let icfg_report = build_icfg(&cfg_ctx, &call_graph, &cfg_results);
        icfg_to_graph(&icfg_report, &symbol_map, &cfg_results, &mut builder);

        let graph_ir = builder.build("test.ts".to_string());

        // Check FunctionEntry/Exit nodes exist for both functions
        let entries: Vec<_> = graph_ir
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::FunctionEntry))
            .collect();
        let exits: Vec<_> = graph_ir
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::FunctionExit))
            .collect();
        assert_eq!(entries.len(), 2, "expected 2 entry nodes");
        assert_eq!(exits.len(), 2, "expected 2 exit nodes");

        // Check ICFG edge types
        let icfg_edges: Vec<_> = graph_ir
            .edges
            .iter()
            .filter(|e| matches!(e.kind, EdgeKind::Icfg { .. }))
            .collect();

        let has_entry_flow = icfg_edges
            .iter()
            .any(|e| matches!(e.kind, EdgeKind::Icfg { icfg_type: IcfgEdgeType::EntryFlow }));
        let has_exit_flow = icfg_edges
            .iter()
            .any(|e| matches!(e.kind, EdgeKind::Icfg { icfg_type: IcfgEdgeType::ExitFlow }));
        let has_call = icfg_edges
            .iter()
            .any(|e| matches!(e.kind, EdgeKind::Icfg { icfg_type: IcfgEdgeType::Call }));
        let has_return = icfg_edges
            .iter()
            .any(|e| matches!(e.kind, EdgeKind::Icfg { icfg_type: IcfgEdgeType::Return }));

        assert!(has_entry_flow, "expected EntryFlow edges");
        assert!(has_exit_flow, "expected ExitFlow edges");
        assert!(has_call, "expected Call edge");
        assert!(has_return, "expected Return edge");

        // Check Contains edges from parent Function to Entry/Exit nodes
        let contains_to_entry = graph_ir.edges.iter().any(|e| {
            matches!(e.kind, EdgeKind::Contains)
                && entries.iter().any(|n| n.id == e.target)
        });
        let contains_to_exit = graph_ir.edges.iter().any(|e| {
            matches!(e.kind, EdgeKind::Contains)
                && exits.iter().any(|n| n.id == e.target)
        });
        assert!(contains_to_entry, "expected Contains edge to FunctionEntry");
        assert!(contains_to_exit, "expected Contains edge to FunctionExit");
    }

    #[test]
    fn icfg_multiple_exits() {
        let report = build_test_icfg(
            r#"
            function classify(x: number) {
              if (x > 0) return "positive";
              if (x < 0) return "negative";
              return "zero";
            }
            "#,
        );
        let func = report.functions.get("classify").unwrap();
        // At least 2 exit blocks (each return path may end in a separate block)
        assert!(
            func.exit_blocks.len() >= 2,
            "expected >= 2 exit blocks, got {}",
            func.exit_blocks.len()
        );
    }
}
