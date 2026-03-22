use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CallKind {
    Direct,
    ThisMethod,
    MemberCall,
    Super,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallSite {
    pub kind: CallKind,
    pub callee_text: String,
    pub target_name: String,
    pub receiver: Option<String>,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CallCategory {
    Domain,
    Infra,
    Builtin,
    Unresolved,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallEdge {
    pub caller: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callee: Option<String>,
    pub kind: CallKind,
    pub target_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_source: Option<String>,
    pub call_category: CallCategory,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntry {
    pub local_name: String,
    pub imported_name: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallGraphData {
    pub nodes: Vec<CallGraphNode>,
    pub edges: Vec<CallEdge>,
    pub imports: Vec<ImportEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallGraphNode {
    pub symbol_name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    pub start_line: u32,
}

#[derive(Debug, Clone)]
pub enum ResolvedTarget {
    Internal(String),
    Import(ImportEntry),
}

impl ResolvedTarget {
    pub fn as_internal_name(&self) -> Option<String> {
        match self {
            ResolvedTarget::Internal(name) => Some(name.clone()),
            ResolvedTarget::Import(_) => None,
        }
    }

    pub fn as_import_source(&self) -> Option<String> {
        match self {
            ResolvedTarget::Internal(_) => None,
            ResolvedTarget::Import(entry) => Some(entry.source.clone()),
        }
    }
}
