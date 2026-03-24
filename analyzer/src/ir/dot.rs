use std::collections::BTreeSet;

use super::graph::*;

/// Render a Graph IR as DOT, filtered to a specific function and its neighborhood.
///
/// Includes: the root function node, all its Contains descendants, 1-hop targets
/// of outgoing edges, and ICFG callee clusters reachable via Icfg::Call edges.
pub fn render_graph_dot(graph: &GraphIR, filter_fn: &str) -> String {
    // Find the root function node
    let root = graph.nodes.iter().find(|n| {
        matches!(n.kind, NodeKind::Function | NodeKind::Method | NodeKind::Class)
            && n.symbol_name.as_deref() == Some(filter_fn)
    });
    let Some(root) = root else {
        return format!("digraph {{\n  // function '{}' not found\n}}\n", filter_fn);
    };
    let root_id = root.id;

    // Collect Contains descendants (BFS)
    let mut core_ids: BTreeSet<NodeId> = BTreeSet::new();
    core_ids.insert(root_id);
    let mut queue: Vec<NodeId> = vec![root_id];
    while let Some(id) = queue.pop() {
        for edge in &graph.edges {
            if edge.source == id && matches!(edge.kind, EdgeKind::Contains) {
                if core_ids.insert(edge.target) {
                    queue.push(edge.target);
                }
            }
        }
    }

    // Collect 1-hop targets of outgoing non-Contains edges from core nodes
    let mut visible_ids = core_ids.clone();
    for edge in &graph.edges {
        if core_ids.contains(&edge.source) && !matches!(edge.kind, EdgeKind::Contains) {
            visible_ids.insert(edge.target);
        }
    }

    // ICFG-aware expansion: for each newly-visible FunctionEntry, find its parent
    // Function via incoming Contains, then BFS that parent's Contains descendants
    // to include the full callee cluster (CfgBlocks, FunctionExit, etc.).
    let new_entries: Vec<NodeId> = visible_ids
        .difference(&core_ids)
        .copied()
        .filter(|&id| {
            graph.nodes.iter().any(|n| n.id == id && matches!(n.kind, NodeKind::FunctionEntry))
        })
        .collect();

    for entry_id in new_entries {
        // Find parent Function node via incoming Contains edge
        if let Some(parent_edge) = graph.edges.iter().find(|e| {
            e.target == entry_id && matches!(e.kind, EdgeKind::Contains)
        }) {
            let parent_id = parent_edge.source;
            // BFS the parent's Contains descendants (use separate visited set
            // because the parent may already be in visible_ids from 1-hop)
            let mut callee_visited = BTreeSet::new();
            let mut callee_queue = vec![parent_id];
            while let Some(id) = callee_queue.pop() {
                if callee_visited.insert(id) {
                    visible_ids.insert(id);
                    for edge in &graph.edges {
                        if edge.source == id && matches!(edge.kind, EdgeKind::Contains) {
                            callee_queue.push(edge.target);
                        }
                    }
                }
            }
        }
    }

    // Also include incoming Icfg::Return edges targeting core nodes
    for edge in &graph.edges {
        if matches!(edge.kind, EdgeKind::Icfg { icfg_type: IcfgEdgeType::Return })
            && core_ids.contains(&edge.target)
            && visible_ids.contains(&edge.source)
        {
            // source is already in visible_ids from callee expansion above
        }
    }

    // Collect visible edges
    let visible_edges: Vec<&GraphEdge> = graph.edges.iter().filter(|e| {
        visible_ids.contains(&e.source) && visible_ids.contains(&e.target)
    }).collect();

    // Build DOT
    let mut dot = String::from("digraph {\n  rankdir=TB;\n  node [fontname=\"Helvetica\"];\n\n");

    // Core subgraph (cluster)
    dot.push_str(&format!("  subgraph \"cluster_{}\" {{\n", escape_dot(filter_fn)));
    dot.push_str(&format!("    label=\"{}\";\n", escape_dot(filter_fn)));
    dot.push_str("    style=dashed;\n");
    for id in &core_ids {
        if let Some(node) = graph.nodes.iter().find(|n| n.id == *id) {
            dot.push_str(&format!("    {};\n", render_node(node)));
        }
    }
    dot.push_str("  }\n\n");

    // External nodes (visible but not in core)
    for id in &visible_ids {
        if core_ids.contains(id) { continue; }
        if let Some(node) = graph.nodes.iter().find(|n| n.id == *id) {
            dot.push_str(&format!("  {};\n", render_node(node)));
        }
    }
    dot.push('\n');

    // Edges
    for edge in &visible_edges {
        // Skip Contains edges in DOT (structure is shown by cluster)
        if matches!(edge.kind, EdgeKind::Contains) { continue; }
        dot.push_str(&format!("  {}", render_edge(edge)));
    }

    dot.push_str("}\n");
    dot
}

fn render_node(node: &GraphNode) -> String {
    let label = if node.label.len() > 40 {
        format!("{}...", &node.label[..37])
    } else {
        node.label.clone()
    };

    let (shape, style) = match node.kind {
        NodeKind::Function | NodeKind::Method => ("box", "filled"),
        NodeKind::Class => ("box", "filled,bold"),
        NodeKind::CfgBlock => ("rectangle", "\"\""),
        NodeKind::CallSite => ("ellipse", "\"\""),
        NodeKind::DecisionPoint => ("diamond", "\"\""),
        NodeKind::ExternalSymbol => ("box", "dashed"),
        NodeKind::DataFlowDef => ("note", "filled"),
        NodeKind::DataFlowUse => ("ellipse", "\"\""),
        NodeKind::FunctionEntry => ("oval", "filled"),
        NodeKind::FunctionExit => ("oval", "filled"),
    };

    let fill = match node.kind {
        NodeKind::Function | NodeKind::Method => ", fillcolor=\"lightyellow\"",
        NodeKind::Class => ", fillcolor=\"lightgrey\"",
        NodeKind::ExternalSymbol => ", fillcolor=\"lightblue\"",
        NodeKind::DataFlowDef => ", fillcolor=\"lightyellow\"",
        NodeKind::FunctionEntry => ", fillcolor=\"palegreen\"",
        NodeKind::FunctionExit => ", fillcolor=\"lightsalmon\"",
        _ => "",
    };

    format!(
        "n{} [label=\"{}\", shape={}, style={}{}]",
        node.id.0,
        escape_dot(&label),
        shape,
        style,
        fill,
    )
}

fn render_edge(edge: &GraphEdge) -> String {
    let (color, style, label) = match &edge.kind {
        EdgeKind::Cfg { cfg_type } => {
            let c = match cfg_type {
                CfgEdgeType::Normal => "black",
                CfgEdgeType::Backedge => "grey",
                CfgEdgeType::Jump => "green",
                CfgEdgeType::Unreachable => "grey",
                CfgEdgeType::Error => "red",
            };
            let s = match cfg_type {
                CfgEdgeType::Unreachable => "dotted",
                _ => "solid",
            };
            (c, s, format!("{:?}", cfg_type))
        }
        EdgeKind::Call { call_kind, .. } => {
            ("blue", "solid", call_kind.clone())
        }
        EdgeKind::Contains => ("grey", "dashed", String::new()),
        EdgeKind::DecisionBranch { branch } => {
            let label = if *branch { "T" } else { "F" };
            ("green", "solid", label.to_string())
        }
        EdgeKind::DataDep { .. } => {
            let label = edge.label.as_deref().unwrap_or("def→use");
            ("orange", "solid", label.to_string())
        }
        EdgeKind::Icfg { icfg_type } => {
            match icfg_type {
                IcfgEdgeType::Call => ("purple", "solid", "icfg:call".to_string()),
                IcfgEdgeType::Return => ("purple", "dashed", "icfg:return".to_string()),
                IcfgEdgeType::EntryFlow => ("darkgreen", "solid", "icfg:entry".to_string()),
                IcfgEdgeType::ExitFlow => ("darkred", "solid", "icfg:exit".to_string()),
            }
        }
    };

    format!(
        "n{} -> n{} [color=\"{}\", style={}, label=\"{}\"]\n",
        edge.source.0,
        edge.target.0,
        color,
        style,
        escape_dot(&label),
    )
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
