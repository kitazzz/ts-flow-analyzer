use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionReport {
    pub symbol_name: String,
    pub symbol_kind: SymbolKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_name: Option<String>,
    pub function_name: String,
    pub file_path: String,
    pub start_line: u32,
    pub metrics: FunctionMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicates: Option<Vec<AtomicPredicate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<Vec<Effect>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_table: Option<DecisionTableData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    Function,
    VariableFunction,
    Method,
    Class,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "function"),
            SymbolKind::VariableFunction => write!(f, "variableFunction"),
            SymbolKind::Method => write!(f, "method"),
            SymbolKind::Class => write!(f, "class"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionMetrics {
    pub if_count: u32,
    pub else_if_count: u32,
    pub switch_count: u32,
    pub ternary_count: u32,
    pub return_count: u32,
    pub max_nesting_depth: u32,
    pub cyclomatic_complexity: u32,
    pub local_function_count: u32,
    pub function_nesting_depth: u32,
    pub callback_nesting_depth: u32,
    pub logical_operator_count: u32,
    pub negation_count: u32,
    pub atomic_condition_count: u32,
    pub max_condition_depth: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomicPredicate {
    pub text: String,
    pub negated: bool,
    pub kind: PredicateKind,
    pub context: DecisionContext,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub true_meaning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub false_meaning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PredicateKind {
    Comparison,
    NullCheck,
    TypeCheck,
    Call,
    Truthiness,
    Negation,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DecisionContext {
    If,
    ElseIf,
    Ternary,
    While,
    DoWhile,
    For,
    Case,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub kind: EffectKind,
    pub side_effect: SideEffectClass,
    pub text: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EffectKind {
    Return,
    Throw,
    Assignment,
    Call,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SideEffectClass {
    DbWrite,
    DbRead,
    ExternalApi,
    Logging,
    Throw,
    Return,
    StateWrite,
    PureCall,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionTableData {
    pub symbol_name: String,
    pub symbol_kind: String,
    pub decisions: Vec<DecisionPoint>,
    pub truth_rows: Vec<TruthRow>,
    pub mcdc_cases: Vec<McdcCase>,
    pub happy_path: Option<TruthRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionPoint {
    pub index: usize,
    pub predicate: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_end: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TruthRow {
    pub values: Vec<String>, // "T", "F", "*"
    pub outcome: String,
    pub outcome_label: String,
    pub outcome_kind: String, // "return", "throw", "happy"
    pub line: u32,
    /// Whether the terminal statement (return/throw) of this path was reached
    /// by the CFG builder. None when CFG is unavailable. Some(false) means the
    /// CFG builder statically marked this block as dead code (e.g. code after
    /// an unconditional return). Does NOT imply row feasibility across all predicates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_reachable: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McdcCase {
    pub predicate: String,
    pub index: usize,
    pub row_false: TruthRow,
    pub row_true: TruthRow,
}
