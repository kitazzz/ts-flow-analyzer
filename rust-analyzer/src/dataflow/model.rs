use serde::Serialize;

pub type DefId = u32;
pub type UseId = u32;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DefKind {
    Declaration,
    Parameter,
    Assignment,
    ForBinding,
    CatchBinding,
    Destructuring,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Def {
    pub id: DefId,
    pub name: String,
    pub kind: DefKind,
    pub line: u32,
    pub span_start: u32,
    pub span_end: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UseKind {
    Read,
    MemberRead,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Use {
    pub id: UseId,
    pub name: String,
    pub kind: UseKind,
    pub line: u32,
    pub span_start: u32,
    pub span_end: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefUseEdge {
    pub def_id: DefId,
    pub use_id: UseId,
    pub def_name: String,
    pub def_line: u32,
    pub use_line: u32,
    pub use_kind: UseKind,
    pub may_reach: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFlowReport {
    pub defs: Vec<Def>,
    pub uses: Vec<Use>,
    pub def_use_edges: Vec<DefUseEdge>,
}
