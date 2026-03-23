use std::collections::{BTreeSet, VecDeque};

use super::model::CallGraphData;

pub fn render_call_graph_dot(graph: &CallGraphData, entrypoint: Option<&str>) -> String {
    let active_nodes: BTreeSet<&str> = if let Some(entry) = entrypoint {
        reachable_from(entry, graph)
    } else {
        graph.nodes.iter().map(|n| n.symbol_name.as_str()).collect()
    };

    let mut dot = String::from("digraph call_graph {\n");
    dot.push_str("    rankdir=LR;\n");
    dot.push_str("    node [shape=box, style=filled, fillcolor=lightyellow];\n\n");

    // Render nodes
    for node in &graph.nodes {
        if !active_nodes.contains(node.symbol_name.as_str()) {
            continue;
        }
        let label = if let Some(ref class) = node.class_name {
            format!("{}\\n({})", node.symbol_name, class)
        } else {
            node.symbol_name.clone()
        };
        dot.push_str(&format!(
            "    \"{}\" [label=\"{}\"]\n",
            escape_dot(&node.symbol_name),
            escape_dot(&label)
        ));
    }

    // Collect import targets for rendering as external nodes
    let mut import_targets: BTreeSet<String> = BTreeSet::new();

    // Render edges
    for edge in &graph.edges {
        if !active_nodes.contains(edge.caller.as_str()) {
            continue;
        }

        if let Some(ref callee) = edge.callee {
            // Resolved internal edge
            if active_nodes.contains(callee.as_str()) {
                dot.push_str(&format!(
                    "    \"{}\" -> \"{}\" [label=\"{}\"]\n",
                    escape_dot(&edge.caller),
                    escape_dot(callee),
                    escape_dot(&edge.target_name),
                ));
            }
        } else if let Some(ref import_source) = edge.import_source {
            // Import edge — dashed, with module path
            let ext_label = format!("{}::{}", import_source, edge.target_name);
            import_targets.insert(ext_label.clone());
            dot.push_str(&format!(
                "    \"{}\" -> \"{}\" [label=\"{}\", style=dashed, color=blue]\n",
                escape_dot(&edge.caller),
                escape_dot(&ext_label),
                escape_dot(&edge.target_name),
            ));
        } else {
            // Unresolved edge — dotted, red
            let receiver_prefix = edge
                .receiver
                .as_deref()
                .map(|r| format!("{}.", r))
                .unwrap_or_default();
            let unresolved_label = format!("{}{}", receiver_prefix, edge.target_name);
            let unresolved_id = format!("unresolved::{}", unresolved_label);
            import_targets.insert(unresolved_id.clone());
            dot.push_str(&format!(
                "    \"{}\" -> \"{}\" [label=\"{}\", style=dotted, color=red]\n",
                escape_dot(&edge.caller),
                escape_dot(&unresolved_id),
                escape_dot(&unresolved_label),
            ));
        }
    }

    // Render external/unresolved nodes
    for target in &import_targets {
        let (style, color) = if target.starts_with("unresolved::") {
            ("dotted", "lightcoral")
        } else {
            ("dashed", "lightblue")
        };
        let label = if let Some(stripped) = target.strip_prefix("unresolved::") {
            stripped
        } else {
            target
        };
        dot.push_str(&format!(
            "    \"{}\" [label=\"{}\", style={}, fillcolor={}]\n",
            escape_dot(target),
            escape_dot(label),
            style,
            color,
        ));
    }

    dot.push_str("}\n");
    dot
}

fn reachable_from<'a>(entry: &str, graph: &'a CallGraphData) -> BTreeSet<&'a str> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([entry.to_string()]);
    while let Some(node) = queue.pop_front() {
        // Only include if it's actually a known node
        let is_known = graph.nodes.iter().any(|n| n.symbol_name == node);
        if !is_known {
            continue;
        }
        let node_ref = graph.nodes.iter().find(|n| n.symbol_name == node).unwrap();
        if !visited.insert(node_ref.symbol_name.as_str()) {
            continue;
        }
        for edge in graph.edges.iter().filter(|e| e.caller == node) {
            if let Some(ref callee) = edge.callee {
                if !visited.contains(callee.as_str()) {
                    queue.push_back(callee.clone());
                }
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
