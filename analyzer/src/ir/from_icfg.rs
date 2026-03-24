use std::collections::HashMap;

use super::builder::GraphBuilder;
use super::from_cfg::CfgBuildResult;
use super::graph::*;
use crate::icfg::model::IcfgReport;

/// Convert an `IcfgReport` into Graph IR nodes and edges.
///
/// Creates `FunctionEntry` / `FunctionExit` nodes for each function with CFG data,
/// connects them to existing `CfgBlock` graph nodes via `EntryFlow` / `ExitFlow` edges,
/// and adds interprocedural `Call` / `Return` edges for resolved same-file calls.
pub fn icfg_to_graph(
    report: &IcfgReport,
    symbol_map: &HashMap<String, NodeId>,
    cfg_results: &HashMap<String, CfgBuildResult>,
    builder: &mut GraphBuilder,
) {
    // Phase 1: Create FunctionEntry and FunctionExit nodes for each function
    let mut entry_node_map: HashMap<String, NodeId> = HashMap::new();
    let mut exit_node_map: HashMap<String, NodeId> = HashMap::new();

    for (symbol_name, func_info) in &report.functions {
        let Some(&parent_id) = symbol_map.get(symbol_name) else {
            continue;
        };
        let Some(cfg_result) = cfg_results.get(symbol_name) else {
            continue;
        };

        // FunctionEntry node
        let entry_id = builder.add_node(
            NodeKind::FunctionEntry,
            format!("entry:{}", symbol_name),
            Some(symbol_name.clone()),
            SourceLoc {
                line: None,
                span_start: None,
                span_end: None,
            },
        );
        builder.add_edge(parent_id, entry_id, EdgeKind::Contains, None);

        // EntryFlow: FunctionEntry → first CfgBlock
        if let Some(&first_block_node) = cfg_result.block_to_node.get(&func_info.entry_block) {
            builder.add_edge(
                entry_id,
                first_block_node,
                EdgeKind::Icfg {
                    icfg_type: IcfgEdgeType::EntryFlow,
                },
                None,
            );
        }
        entry_node_map.insert(symbol_name.clone(), entry_id);

        // FunctionExit node (one per function)
        let exit_id = builder.add_node(
            NodeKind::FunctionExit,
            format!("exit:{}", symbol_name),
            Some(symbol_name.clone()),
            SourceLoc {
                line: None,
                span_start: None,
                span_end: None,
            },
        );
        builder.add_edge(parent_id, exit_id, EdgeKind::Contains, None);

        // ExitFlow: terminal CfgBlocks → FunctionExit
        for &exit_block in &func_info.exit_blocks {
            if let Some(&exit_block_node) = cfg_result.block_to_node.get(&exit_block) {
                builder.add_edge(
                    exit_block_node,
                    exit_id,
                    EdgeKind::Icfg {
                        icfg_type: IcfgEdgeType::ExitFlow,
                    },
                    None,
                );
            }
        }
        exit_node_map.insert(symbol_name.clone(), exit_id);
    }

    // Phase 2: Create interprocedural Call and Return edges
    for conn in &report.connections {
        let Some(caller_cfg) = cfg_results.get(&conn.caller_symbol) else {
            continue;
        };
        let Some(&callee_entry_id) = entry_node_map.get(&conn.callee_symbol) else {
            continue;
        };

        // Call edge: call-site CfgBlock → callee FunctionEntry
        if let Some(&call_block_node) = caller_cfg.block_to_node.get(&conn.call_site_block) {
            builder.add_edge(
                call_block_node,
                callee_entry_id,
                EdgeKind::Icfg {
                    icfg_type: IcfgEdgeType::Call,
                },
                None,
            );
        }

        // Return edge: callee FunctionExit → caller return-site CfgBlocks
        if let Some(&callee_exit_id) = exit_node_map.get(&conn.callee_symbol) {
            for &return_block in &conn.return_site_blocks {
                if let Some(&return_block_node) = caller_cfg.block_to_node.get(&return_block) {
                    builder.add_edge(
                        callee_exit_id,
                        return_block_node,
                        EdgeKind::Icfg {
                            icfg_type: IcfgEdgeType::Return,
                        },
                        None,
                    );
                }
            }
        }
    }
}
