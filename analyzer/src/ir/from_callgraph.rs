use std::collections::HashMap;

use crate::callgraph::model::CallGraphData;

use super::builder::GraphBuilder;
use super::graph::*;

pub fn callgraph_to_graph(
    cg: &CallGraphData,
    symbol_map: &HashMap<String, NodeId>,
    builder: &mut GraphBuilder,
) {
    let mut external_map: HashMap<String, NodeId> = HashMap::new();

    for edge in &cg.edges {
        // Skip if caller not in symbol_map
        let Some(&caller_id) = symbol_map.get(&edge.caller) else {
            continue;
        };

        // Create CallSite node
        let cs_id = builder.add_node(
            NodeKind::CallSite,
            edge.target_name.clone(),
            None,
            SourceLoc {
                line: Some(edge.line),
                span_start: edge.span_start,
                span_end: edge.span_end,
            },
        );

        // caller -> CallSite (Contains)
        builder.add_edge(caller_id, cs_id, EdgeKind::Contains, None);

        // Resolve callee
        let callee_id = if let Some(callee_name) = &edge.callee {
            // Internal callee
            symbol_map.get(callee_name).copied()
        } else {
            None
        };

        let target_id = if let Some(id) = callee_id {
            id
        } else if let Some(import_source) = &edge.import_source {
            // Import external symbol
            let imported = edge
                .imported_name
                .as_deref()
                .unwrap_or(&edge.target_name);
            let dedup_key = format!("{}::{}", import_source, imported);
            *external_map.entry(dedup_key.clone()).or_insert_with(|| {
                let id = builder.add_node(
                    NodeKind::ExternalSymbol,
                    format!("{}::{}", import_source, imported),
                    Some(dedup_key),
                    SourceLoc {
                        line: None,
                        span_start: None,
                        span_end: None,
                    },
                );
                builder.nodes.last_mut().unwrap().properties.insert(
                    "importSource".to_string(),
                    serde_json::Value::String(import_source.clone()),
                );
                builder.nodes.last_mut().unwrap().properties.insert(
                    "resolution".to_string(),
                    serde_json::Value::String("import".to_string()),
                );
                id
            })
        } else {
            // Unresolved
            let receiver_prefix = edge
                .receiver
                .as_deref()
                .map(|r| format!("{}.", r))
                .unwrap_or_default();
            let dedup_key = format!("unresolved::{}{}", receiver_prefix, edge.target_name);
            *external_map.entry(dedup_key.clone()).or_insert_with(|| {
                let id = builder.add_node(
                    NodeKind::ExternalSymbol,
                    format!("{}{}", receiver_prefix, edge.target_name),
                    Some(dedup_key),
                    SourceLoc {
                        line: None,
                        span_start: None,
                        span_end: None,
                    },
                );
                builder.nodes.last_mut().unwrap().properties.insert(
                    "resolution".to_string(),
                    serde_json::Value::String("unresolved".to_string()),
                );
                if let Some(receiver) = &edge.receiver {
                    builder.nodes.last_mut().unwrap().properties.insert(
                        "receiver".to_string(),
                        serde_json::Value::String(receiver.clone()),
                    );
                }
                id
            })
        };

        // CallSite -> callee (Call)
        builder.add_edge(
            cs_id,
            target_id,
            EdgeKind::Call {
                call_kind: edge.kind.to_string(),
                category: edge.call_category.to_string(),
            },
            None,
        );
    }
}
