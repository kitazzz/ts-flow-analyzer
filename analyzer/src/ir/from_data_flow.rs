use std::collections::HashMap;

use super::builder::GraphBuilder;
use super::graph::*;
use crate::dataflow::model::{DataFlowReport, DefId, UseId};

pub fn data_flow_to_graph(
    report: &DataFlowReport,
    parent_id: NodeId,
    builder: &mut GraphBuilder,
) {
    let mut def_node_map: HashMap<DefId, NodeId> = HashMap::new();
    for def in &report.defs {
        let label = format!("def:{} ({:?})", def.name, def.kind);
        let id = builder.add_node(
            NodeKind::DataFlowDef,
            label,
            None,
            SourceLoc {
                line: Some(def.line),
                span_start: Some(def.span_start),
                span_end: Some(def.span_end),
            },
        );
        builder.add_edge(parent_id, id, EdgeKind::Contains, None);
        def_node_map.insert(def.id, id);
    }

    let mut use_node_map: HashMap<UseId, NodeId> = HashMap::new();
    for u in &report.uses {
        let label = format!("use:{} ({:?})", u.name, u.kind);
        let id = builder.add_node(
            NodeKind::DataFlowUse,
            label,
            None,
            SourceLoc {
                line: Some(u.line),
                span_start: Some(u.span_start),
                span_end: Some(u.span_end),
            },
        );
        builder.add_edge(parent_id, id, EdgeKind::Contains, None);
        use_node_map.insert(u.id, id);
    }

    for edge in &report.def_use_edges {
        if let (Some(&def_node), Some(&use_node)) = (
            def_node_map.get(&edge.def_id),
            use_node_map.get(&edge.use_id),
        ) {
            let label = if edge.may_reach {
                Some("may".to_string())
            } else {
                None
            };
            builder.add_edge(
                def_node,
                use_node,
                EdgeKind::DataDep {
                    dep_kind: DataDepKind::DefUse,
                },
                label,
            );
        }
    }
}
