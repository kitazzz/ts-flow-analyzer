use std::collections::{BTreeSet, VecDeque};

use super::model::IcfgReport;

/// Render a high-level ICFG DOT visualization.
///
/// Shows function-level structure with entry/body/exit nodes per function cluster,
/// and interprocedural call/return edges between them.
///
/// If `entrypoint` is `Some`, BFS from that function to show only reachable functions.
/// If `None`, show all functions.
pub fn render_icfg_dot(report: &IcfgReport, entrypoint: Option<&str>) -> String {
    let active_functions: BTreeSet<&str> = if let Some(entry) = entrypoint {
        reachable_from(entry, report)
    } else {
        report.functions.keys().map(|s| s.as_str()).collect()
    };

    let mut dot = String::from("digraph icfg {\n");
    dot.push_str("    rankdir=TB;\n");
    dot.push_str("    compound=true;\n");
    dot.push_str("    fontname=\"Helvetica\";\n");
    dot.push_str("    node [fontname=\"Helvetica\"];\n");
    dot.push_str("    edge [fontname=\"Helvetica\"];\n\n");

    // Render function clusters (sorted for deterministic output)
    for &func_name in &active_functions {
        let func_info = match report.functions.get(func_name) {
            Some(info) => info,
            None => continue,
        };
        let escaped = escape_dot(func_name);
        let exit_count = func_info.exit_blocks.len();
        let body_label = if exit_count > 1 {
            format!("{} ({} exits)", escaped, exit_count)
        } else {
            escaped.clone()
        };

        dot.push_str(&format!(
            "    subgraph \"cluster_{}\" {{\n",
            escape_dot(func_name)
        ));
        dot.push_str(&format!("        label=\"{}\";\n", escaped));
        dot.push_str("        style=rounded;\n");
        dot.push_str("        color=grey60;\n\n");

        // Entry node
        dot.push_str(&format!(
            "        \"{}::entry\" [label=\"entry\", shape=oval, style=filled, fillcolor=palegreen];\n",
            escaped
        ));
        // Body node
        dot.push_str(&format!(
            "        \"{}::body\" [label=\"{}\", shape=rectangle, style=filled, fillcolor=lightyellow];\n",
            escaped, body_label
        ));
        // Exit node
        dot.push_str(&format!(
            "        \"{}::exit\" [label=\"exit\", shape=oval, style=filled, fillcolor=lightsalmon];\n",
            escaped
        ));

        // Internal edges (grey)
        dot.push_str(&format!(
            "        \"{}::entry\" -> \"{}::body\" [color=grey];\n",
            escaped, escaped
        ));
        dot.push_str(&format!(
            "        \"{}::body\" -> \"{}::exit\" [color=grey];\n",
            escaped, escaped
        ));

        dot.push_str("    }\n\n");
    }

    // Collect and sort interprocedural edges for deterministic output
    let mut call_edges: Vec<(&str, &str, usize)> = Vec::new();
    for conn in &report.connections {
        if !active_functions.contains(conn.caller_symbol.as_str()) {
            continue;
        }
        if !active_functions.contains(conn.callee_symbol.as_str()) {
            continue;
        }
        call_edges.push((
            &conn.caller_symbol,
            &conn.callee_symbol,
            conn.call_site_block.index(),
        ));
    }
    call_edges.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(b.1)).then(a.2.cmp(&b.2)));

    // Render interprocedural edges (purple)
    for (caller, callee, _) in &call_edges {
        let caller_esc = escape_dot(caller);
        let callee_esc = escape_dot(callee);

        // Call edge: caller::body → callee::entry (solid, purple)
        dot.push_str(&format!(
            "    \"{}::body\" -> \"{}::entry\" [color=purple, penwidth=1.5];\n",
            caller_esc, callee_esc
        ));
        // Return edge: callee::exit → caller::body (dashed, purple)
        dot.push_str(&format!(
            "    \"{}::exit\" -> \"{}::body\" [color=purple, style=dashed, penwidth=1.5];\n",
            callee_esc, caller_esc
        ));
    }

    dot.push_str("}\n");
    dot
}

/// BFS from entrypoint following IcfgConnections to find reachable function names.
fn reachable_from<'a>(entry: &str, report: &'a IcfgReport) -> BTreeSet<&'a str> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([entry.to_string()]);
    while let Some(node) = queue.pop_front() {
        // Only include if it's actually a known function
        let Some((name, _)) = report.functions.get_key_value(&node) else {
            continue;
        };
        if !visited.insert(name.as_str()) {
            continue;
        }
        for conn in &report.connections {
            if conn.caller_symbol == node && !visited.contains(conn.callee_symbol.as_str()) {
                queue.push_back(conn.callee_symbol.clone());
            }
        }
    }
    visited
}

fn escape_dot(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use oxc_cfg::BlockNodeId;

    use crate::icfg::model::*;

    use super::*;

    /// Helper: build a minimal IcfgReport for testing
    fn make_report(
        funcs: Vec<(&str, usize, Vec<usize>)>,
        conns: Vec<(&str, &str, usize)>,
    ) -> IcfgReport {
        let mut functions = HashMap::new();
        for (name, entry, exits) in funcs {
            functions.insert(
                name.to_string(),
                FunctionIcfgInfo {
                    entry_block: BlockNodeId::new(entry),
                    exit_blocks: exits.into_iter().map(BlockNodeId::new).collect(),
                },
            );
        }
        let connections = conns
            .into_iter()
            .map(|(caller, callee, block)| IcfgConnection {
                caller_symbol: caller.to_string(),
                callee_symbol: callee.to_string(),
                call_site_block: BlockNodeId::new(block),
                return_site_blocks: vec![BlockNodeId::new(block + 1)],
            })
            .collect();
        IcfgReport {
            functions,
            connections,
        }
    }

    #[test]
    fn icfg_dot_renders_all_functions() {
        let report = make_report(
            vec![("caller", 0, vec![2]), ("callee", 3, vec![5])],
            vec![("caller", "callee", 1)],
        );

        let dot = render_icfg_dot(&report, None);

        // Both clusters present
        assert!(dot.contains("cluster_caller"), "missing caller cluster");
        assert!(dot.contains("cluster_callee"), "missing callee cluster");

        // Entry/body/exit nodes
        assert!(dot.contains("\"caller::entry\""));
        assert!(dot.contains("\"caller::body\""));
        assert!(dot.contains("\"caller::exit\""));
        assert!(dot.contains("\"callee::entry\""));
        assert!(dot.contains("\"callee::body\""));
        assert!(dot.contains("\"callee::exit\""));

        // Call edge
        assert!(dot.contains("\"caller::body\" -> \"callee::entry\""));
        // Return edge
        assert!(dot.contains("\"callee::exit\" -> \"caller::body\""));
    }

    #[test]
    fn icfg_dot_filters_by_entrypoint() {
        let report = make_report(
            vec![
                ("main", 0, vec![2]),
                ("helper", 3, vec![5]),
                ("orphan", 6, vec![8]),
            ],
            vec![("main", "helper", 1)],
        );

        let dot = render_icfg_dot(&report, Some("main"));

        // main and helper present
        assert!(dot.contains("cluster_main"));
        assert!(dot.contains("cluster_helper"));
        // orphan excluded
        assert!(!dot.contains("cluster_orphan"));
    }

    #[test]
    fn icfg_dot_unreachable_excluded() {
        let report = make_report(
            vec![("main", 0, vec![2]), ("orphan", 3, vec![5])],
            vec![], // no connections
        );

        let dot = render_icfg_dot(&report, Some("main"));

        assert!(dot.contains("cluster_main"));
        assert!(!dot.contains("cluster_orphan"));
    }

    #[test]
    fn icfg_dot_multiple_exits_annotated() {
        let report = make_report(
            vec![("classify", 0, vec![2, 3, 4])],
            vec![],
        );

        let dot = render_icfg_dot(&report, None);

        // Body label should contain exit count
        assert!(
            dot.contains("3 exits"),
            "expected '3 exits' in body label, got:\n{}",
            dot
        );
    }

    #[test]
    fn icfg_dot_recursive_call() {
        let report = make_report(
            vec![("fib", 0, vec![3])],
            vec![("fib", "fib", 1), ("fib", "fib", 2)],
        );

        let dot = render_icfg_dot(&report, None);

        // Self-referencing call edge
        assert!(dot.contains("\"fib::body\" -> \"fib::entry\""));
        // Self-referencing return edge
        assert!(dot.contains("\"fib::exit\" -> \"fib::body\""));
    }

    #[test]
    fn icfg_dot_integration() {
        use oxc_allocator::Allocator;
        use oxc_parser::Parser;
        use oxc_span::SourceType;

        use crate::ast::collect_functions::{collect_functions, FunctionNode};
        use crate::callgraph::build_call_graph;
        use crate::control_flow::build_cfg_context;
        use crate::icfg::build::build_icfg;
        use crate::ir::from_cfg::cfg_to_graph;

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
        let dot = render_icfg_dot(&icfg_report, Some("caller"));

        // Both clusters present
        assert!(dot.contains("cluster_caller"), "missing caller cluster");
        assert!(dot.contains("cluster_callee"), "missing callee cluster");

        // Call and return edges
        assert!(dot.contains("\"caller::body\" -> \"callee::entry\""));
        assert!(dot.contains("\"callee::exit\" -> \"caller::body\""));
    }
}
