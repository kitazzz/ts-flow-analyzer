use std::collections::HashMap;

use oxc_cfg::BlockNodeId;

/// Per-function entry/exit block information for ICFG construction.
#[derive(Debug)]
pub struct FunctionIcfgInfo {
    pub entry_block: BlockNodeId,
    pub exit_blocks: Vec<BlockNodeId>,
}

/// An interprocedural connection from a call site in one function
/// to the entry/exit of another resolved internal function.
#[derive(Debug)]
pub struct IcfgConnection {
    /// Caller function's symbol_name
    pub caller_symbol: String,
    /// Callee function's symbol_name (resolved via CallEdge.callee)
    pub callee_symbol: String,
    /// The CFG block in the caller that contains the call expression
    pub call_site_block: BlockNodeId,
    /// Successor blocks in the caller after the call returns (flow edges only)
    pub return_site_blocks: Vec<BlockNodeId>,
}

/// Full single-file ICFG report. Internal model (not serialized).
/// Converted to Graph IR nodes/edges by `ir::from_icfg`.
#[derive(Debug)]
pub struct IcfgReport {
    /// Per-function entry/exit info, keyed by symbol_name
    pub functions: HashMap<String, FunctionIcfgInfo>,
    /// Interprocedural call/return connections
    pub connections: Vec<IcfgConnection>,
}
