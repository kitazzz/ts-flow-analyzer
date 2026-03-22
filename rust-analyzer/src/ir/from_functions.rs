use std::collections::HashMap;

use crate::ast::collect_functions::CollectedFunction;
use crate::model::SymbolKind;

use super::builder::GraphBuilder;
use super::graph::*;

pub fn functions_to_graph(
    collected: &[CollectedFunction<'_>],
    builder: &mut GraphBuilder,
) -> HashMap<String, NodeId> {
    let mut symbol_map: HashMap<String, NodeId> = HashMap::new();
    // Track class nodes for Contains edges
    let mut class_map: HashMap<String, NodeId> = HashMap::new();

    for func in collected {
        let kind = match func.symbol_kind {
            SymbolKind::Class => NodeKind::Class,
            SymbolKind::Method => NodeKind::Method,
            SymbolKind::Function | SymbolKind::VariableFunction => NodeKind::Function,
        };

        let span = func.node.span();
        let loc = SourceLoc {
            line: Some(func.start_line),
            span_start: Some(span.start),
            span_end: Some(span.end),
        };

        let id = builder.add_node(
            kind,
            func.symbol_name.clone(),
            Some(func.symbol_name.clone()),
            loc,
        );
        symbol_map.insert(func.symbol_name.clone(), id);

        // Track class nodes
        if matches!(func.symbol_kind, SymbolKind::Class) {
            if let Some(class_name) = &func.class_name {
                class_map.insert(class_name.clone(), id);
            }
            // Also use symbol_name as class name lookup
            class_map.insert(func.symbol_name.clone(), id);
        }
    }

    // Add Contains edges from Class to its Methods
    for func in collected {
        if matches!(func.symbol_kind, SymbolKind::Method) {
            if let Some(class_name) = &func.class_name {
                if let (Some(&class_id), Some(&method_id)) =
                    (class_map.get(class_name), symbol_map.get(&func.symbol_name))
                {
                    builder.add_edge(class_id, method_id, EdgeKind::Contains, None);
                }
            }
        }
    }

    symbol_map
}
