use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rf-analyze", about = "Recast Forge static analyzer")]
pub struct Cli {
    /// Input file to analyze
    pub file: String,

    /// Output JSON
    #[arg(long)]
    pub json: bool,

    /// Include metrics (default: yes)
    #[arg(long, default_value_t = true)]
    pub metrics: bool,

    /// Include predicates
    #[arg(long)]
    pub predicates: bool,

    /// Include effects
    #[arg(long)]
    pub effects: bool,

    /// Include decision tables
    #[arg(long)]
    pub decision: bool,

    /// Include everything (predicates + effects + decision tables)
    #[arg(long)]
    pub all: bool,

    /// Filter to specific function name
    #[arg(long)]
    pub function: Option<String>,

    /// Enable CFG-enhanced decision table (reachability annotation + &&/|| expansion).
    /// Requires building with --features cfg-analysis for reachability; &&/|| expansion works without it.
    #[arg(long)]
    pub decision_enhanced: bool,

    /// Output DOT format of the CFG to stdout and exit.
    /// Requires building with --features cfg-analysis.
    #[arg(long, value_name = "FUNCTION")]
    pub cfg_dot: Option<String>,

    /// Include call graph in output
    #[arg(long)]
    pub call_graph: bool,

    /// Output call graph as DOT to stdout and exit.
    /// Optional: specify entrypoint function to show only reachable subgraph.
    #[arg(long, value_name = "FUNCTION")]
    pub call_graph_dot: Option<String>,

    /// Include builtin/collection method calls in call graph (hidden by default)
    #[arg(long)]
    pub include_builtin_calls: bool,
}
