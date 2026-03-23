use std::collections::{HashMap, HashSet};

use crate::model::DecisionTableData;

use super::builder::GraphBuilder;
use super::graph::*;

pub fn decision_to_graph(
    dt: &DecisionTableData,
    parent_id: NodeId,
    builder: &mut GraphBuilder,
) {
    if dt.decisions.is_empty() {
        return;
    }

    // Create DecisionPoint nodes
    let mut dp_nodes: Vec<(usize, NodeId)> = Vec::new();
    for dp in &dt.decisions {
        let id = builder.add_node(
            NodeKind::DecisionPoint,
            dp.predicate.clone(),
            None,
            SourceLoc {
                line: Some(dp.line),
                span_start: dp.span_start,
                span_end: dp.span_end,
            },
        );
        builder.add_edge(parent_id, id, EdgeKind::Contains, None);
        dp_nodes.push((dp.index, id));
    }

    let dp_map: HashMap<usize, NodeId> = dp_nodes.iter().copied().collect();

    // Derive DecisionBranch edges from truth rows.
    // For each truth row, find consecutive decision assignments and create
    // edges between them. If row has [T, *, F, T], then:
    //   dp0 --T--> dp2 (dp1 is *, skipped)
    //   dp2 --F--> dp3
    // This captures the actual evaluation order seen in the truth table.
    let mut seen_edges: HashSet<(NodeId, NodeId, bool)> = HashSet::new();

    for row in &dt.truth_rows {
        // Collect the decision indices that have non-* values in this row, in order
        let assigned: Vec<(usize, bool)> = row.values.iter().enumerate()
            .filter_map(|(i, v)| match v.as_str() {
                "T" => Some((i, true)),
                "F" => Some((i, false)),
                _ => None,
            })
            .collect();

        // Link consecutive assigned decisions
        for window in assigned.windows(2) {
            let (from_idx, from_branch) = window[0];
            let (to_idx, _) = window[1];
            if let (Some(&from_id), Some(&to_id)) = (dp_map.get(&from_idx), dp_map.get(&to_idx)) {
                if seen_edges.insert((from_id, to_id, from_branch)) {
                    builder.add_edge(
                        from_id,
                        to_id,
                        EdgeKind::DecisionBranch { branch: from_branch },
                        None,
                    );
                }
            }
        }
    }
}
