use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphIR {
    pub file_path: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,
    pub loc: SourceLoc,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Function,
    Class,
    Method,
    CfgBlock,
    CallSite,
    DecisionPoint,
    ExternalSymbol,
    DataFlowDef,
    DataFlowUse,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLoc {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_end: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum EdgeKind {
    #[serde(rename_all = "camelCase")]
    Cfg { cfg_type: CfgEdgeType },
    #[serde(rename_all = "camelCase")]
    Call { call_kind: String, category: String },
    Contains,
    #[serde(rename_all = "camelCase")]
    DecisionBranch { branch: bool },
    #[serde(rename_all = "camelCase")]
    DataDep { dep_kind: DataDepKind },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DataDepKind {
    DefUse,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CfgEdgeType {
    Normal,
    Backedge,
    Jump,
    Unreachable,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, serde_json::Value>,
}
