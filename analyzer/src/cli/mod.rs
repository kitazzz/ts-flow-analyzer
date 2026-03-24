use clap::Parser;

const AFTER_HELP: &str = "\
Quick start:
  ts-flow-analyzer <FILE>
  ts-flow-analyzer <FILE> --all --json
  ts-flow-analyzer <FILE> --graph --decision --data-flow
  ts-flow-analyzer <FILE> --graph-dot 'ClassName#methodName'
  ts-flow-analyzer <FILE> --icfg
  ts-flow-analyzer <FILE> --icfg-dot 'ClassName#methodName'

Notes:
  Metrics are always included.
  --all enables predicates, effects, data-flow, decision tables, and call graph.
  --graph is separate because it changes the top-level JSON shape.
  --icfg implies --graph (which implies --json). Requires --features cfg-analysis.
  --icfg-dot renders a high-level interprocedural DOT. Requires --features cfg-analysis.
";

#[derive(Parser, Debug)]
#[command(
    name = "ts-flow-analyzer",
    about = "TypeScript flow analyzer",
    next_line_help = true,
    after_help = AFTER_HELP
)]
pub struct Cli {
    /// Input file to analyze
    #[arg(value_name = "FILE", help_heading = "Input")]
    pub file: String,

    /// Path to analyzer config YAML. If omitted, ./analyze.config.yaml
    /// (or legacy ./config.yaml) is loaded when present.
    #[arg(long, value_name = "PATH", help_heading = "Input")]
    pub config: Option<String>,

    /// Output JSON instead of human-readable text
    #[arg(long, help_heading = "Output & Scope")]
    pub json: bool,

    /// Only analyze a specific symbol (for example ClassName#methodName)
    #[arg(long, value_name = "FUNCTION", help_heading = "Output & Scope")]
    pub function: Option<String>,

    /// Enable the common analysis set (predicates, effects, data-flow, decision tables, call graph)
    #[arg(long, help_heading = "Analysis Layers")]
    pub all: bool,

    /// Include predicates
    #[arg(long, help_heading = "Analysis Layers")]
    pub predicates: bool,

    /// Include effects
    #[arg(long, help_heading = "Analysis Layers")]
    pub effects: bool,

    /// Include local def-use / data-flow analysis
    #[arg(long, help_heading = "Analysis Layers")]
    pub data_flow: bool,

    /// Include decision tables
    #[arg(long, help_heading = "Analysis Layers")]
    pub decision: bool,

    /// Include the intra-file call graph in JSON output
    #[arg(long, help_heading = "Analysis Layers")]
    pub call_graph: bool,

    /// Include unified Graph IR in JSON output (implies --json)
    #[arg(long, help_heading = "Analysis Layers")]
    pub graph: bool,

    /// Include ICFG (interprocedural control flow graph) in Graph IR output.
    /// Adds function entry/exit nodes and call/return edges for resolved same-file calls.
    /// Standalone: implies --graph (which implies --json). Requires --features cfg-analysis.
    #[arg(long, help_heading = "Analysis Layers")]
    pub icfg: bool,

    /// Enable CFG-enhanced decision table (reachability annotation + &&/|| expansion).
    /// Requires building with --features cfg-analysis for reachability; &&/|| expansion works without it.
    #[arg(long, help_heading = "Advanced")]
    pub decision_enhanced: bool,

    /// Output DOT format of the CFG to stdout and exit.
    /// Requires building with --features cfg-analysis.
    #[arg(long, value_name = "FUNCTION", help_heading = "Visualization (DOT)")]
    pub cfg_dot: Option<String>,

    /// Output call graph as DOT to stdout and exit.
    /// Optional: specify entrypoint function to show only reachable subgraph.
    #[arg(long, value_name = "FUNCTION", help_heading = "Visualization (DOT)")]
    pub call_graph_dot: Option<String>,

    /// Include builtin/collection method calls in the call graph (hidden by default)
    #[arg(long, help_heading = "Advanced")]
    pub include_builtin_calls: bool,

    /// Output high-level ICFG as DOT to stdout and exit.
    /// Shows function-level entry/body/exit with call/return edges.
    /// Optional: specify entrypoint function to show only reachable subgraph.
    /// Requires --features cfg-analysis.
    #[arg(long, value_name = "FUNCTION", help_heading = "Visualization (DOT)")]
    pub icfg_dot: Option<String>,

    /// Output unified Graph IR as DOT to stdout and exit
    #[arg(long, value_name = "FUNCTION", help_heading = "Visualization (DOT)")]
    pub graph_dot: Option<String>,
}
