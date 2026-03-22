use oxc_ast::ast::*;
use oxc_span::{GetSpan, Span};
use std::collections::HashMap;
use crate::model::{DecisionPoint, DecisionTableData, TruthRow};
use crate::ast::collect_functions::FunctionNode;
use super::mcdc::build_mcdc_cases;

pub fn build_decision_table<'a>(
    node: &FunctionNode<'a>,
    symbol_name: &str,
    symbol_kind: &str,
    source: &str,
    enhanced: bool,
    #[cfg(feature = "cfg-analysis")]
    cfg_ctx: Option<&crate::control_flow::context::CfgContext<'_>>,
) -> Option<DecisionTableData> {
    if matches!(node, FunctionNode::Class(_)) {
        return None;
    }

    let stmts = get_root_statements(node)?;
    if stmts.is_empty() {
        return None;
    }

    let mut decisions: Vec<DecisionPoint> = Vec::new();
    let initial_path = HashMap::new();
    let walk = walk_statements(stmts, vec![initial_path], &mut decisions, source, enhanced);

    if walk.terminated.is_empty() || decisions.is_empty() {
        return None;
    }

    let success_line = find_success_line(&walk.terminated);
    let mut truth_rows: Vec<TruthRow> = walk
        .terminated
        .iter()
        .map(|path| to_truth_row(
            path,
            decisions.len(),
            success_line,
            #[cfg(feature = "cfg-analysis")]
            cfg_ctx,
        ))
        .collect();
    dedupe_rows(&mut truth_rows);

    if truth_rows.is_empty() {
        return None;
    }

    let happy_rows: Vec<&TruthRow> = truth_rows
        .iter()
        .filter(|r| r.outcome_kind == "happy")
        .collect();
    let happy_path = happy_rows
        .into_iter()
        .max_by_key(|r| r.values.iter().filter(|v| *v != "*").count())
        .cloned();

    let mcdc_cases = build_mcdc_cases(&decisions, &truth_rows);

    Some(DecisionTableData {
        symbol_name: symbol_name.to_string(),
        symbol_kind: symbol_kind.to_string(),
        decisions,
        truth_rows,
        mcdc_cases,
        happy_path,
    })
}

fn get_root_statements<'a>(node: &'a FunctionNode<'a>) -> Option<&'a [Statement<'a>]> {
    match node {
        FunctionNode::Function(f) => {
            f.body.as_ref().map(|b| b.statements.as_slice())
        }
        FunctionNode::Arrow(arrow) => {
            Some(arrow.body.statements.as_slice())
        }
        FunctionNode::Class(_) => None,
    }
}

type PathState = HashMap<usize, &'static str>; // index -> "T" or "F"

#[derive(Clone)]
struct PathOutcome {
    assignments: PathState,
    outcome: String,
    outcome_kind: String,
    line: u32,
    #[cfg_attr(not(feature = "cfg-analysis"), allow(dead_code))]
    terminal_span: Option<Span>,
}

struct WalkResult {
    terminated: Vec<PathOutcome>,
    continued: Vec<PathState>,
}

/// Walk a list of statements, processing ALL active paths together so that
/// decisions for each `if` statement are registered exactly once even when
/// multiple paths converge on the same statement.
fn walk_statements<'a>(
    stmts: &'a [Statement<'a>],
    current_paths: Vec<PathState>,
    decisions: &mut Vec<DecisionPoint>,
    source: &str,
    enhanced: bool,
) -> WalkResult {
    let mut continued = current_paths;
    let mut terminated = Vec::new();

    for stmt in stmts {
        if continued.is_empty() {
            break;
        }
        match stmt {
            Statement::IfStatement(if_stmt) => {
                // Process all active paths through this if together so the
                // decision is registered exactly once.
                let (term, cont) =
                    process_if_for_all_paths(if_stmt, continued, decisions, source, enhanced);
                terminated.extend(term);
                continued = cont;
            }
            Statement::BlockStatement(block) => {
                let result =
                    walk_statements(&block.body, continued, decisions, source, enhanced);
                terminated.extend(result.terminated);
                continued = result.continued;
            }
            _ => {
                let mut next_continued = Vec::new();
                for path in continued {
                    let result = walk_simple_statement(stmt, path, source);
                    terminated.extend(result.terminated);
                    next_continued.extend(result.continued);
                }
                continued = next_continued;
            }
        }
    }

    WalkResult { terminated, continued }
}

/// Handle a non-branching statement for a single path.
fn walk_simple_statement(stmt: &Statement<'_>, path: PathState, source: &str) -> WalkResult {
    match stmt {
        Statement::ReturnStatement(ret) => {
            let text = &source[ret.span.start as usize..ret.span.end as usize];
            let line = span_line(source, ret.span.start);
            WalkResult {
                terminated: vec![PathOutcome {
                    assignments: path,
                    outcome: compact(text),
                    outcome_kind: "return".to_string(),
                    line,
                    terminal_span: Some(ret.span),
                }],
                continued: vec![],
            }
        }
        Statement::ThrowStatement(thr) => {
            let text = &source[thr.span.start as usize..thr.span.end as usize];
            let line = span_line(source, thr.span.start);
            WalkResult {
                terminated: vec![PathOutcome {
                    assignments: path,
                    outcome: compact(text),
                    outcome_kind: "throw".to_string(),
                    line,
                    terminal_span: Some(thr.span),
                }],
                continued: vec![],
            }
        }
        _ => WalkResult {
            terminated: vec![],
            continued: vec![path],
        },
    }
}

/// Process an `if` statement for all active paths together.
/// Decisions are registered exactly once regardless of how many paths are active.
fn process_if_for_all_paths<'a>(
    if_node: &'a IfStatement<'a>,
    paths: Vec<PathState>,
    decisions: &mut Vec<DecisionPoint>,
    source: &str,
    enhanced: bool,
) -> (Vec<PathOutcome>, Vec<PathState>) {
    let mut then_paths: Vec<PathState> = Vec::new();
    let mut else_paths: Vec<PathState> = Vec::new();

    if enhanced && is_boolean_logical(&if_node.test) {
        // O6c: expand && / || into short-circuit rows.
        // ?? (Coalesce) is intentionally excluded — it has nullish semantics, not boolean,
        // and is treated as a single opaque predicate like any other expression.
        let start_idx = decisions.len();
        collect_atomics(&if_node.test, source, decisions);

        for path in paths {
            for (branch_path, is_then) in gen_paths(&if_node.test, decisions, start_idx, path) {
                if is_then {
                    then_paths.push(branch_path);
                } else {
                    else_paths.push(branch_path);
                }
            }
        }
    } else {
        // Original: register one decision ONCE, then fork each path
        let index = decisions.len();
        let pred_text =
            &source[if_node.test.span().start as usize..if_node.test.span().end as usize];
        let line = span_line(source, if_node.span.start);
        decisions.push(DecisionPoint {
            index,
            predicate: compact(pred_text),
            line,
        });

        for path in paths {
            let mut then_path = path.clone();
            then_path.insert(index, "T");
            then_paths.push(then_path);

            let mut else_path = path;
            else_path.insert(index, "F");
            else_paths.push(else_path);
        }
    }

    // Process the then-branch with all then-paths together
    let then_result =
        walk_branch_for_all_paths(&if_node.consequent, then_paths, decisions, source, enhanced);
    let mut terminated = then_result.terminated;
    let mut continued = then_result.continued;

    // Process the else-branch (or propagate else-paths as-is if no alternate)
    if let Some(alt) = &if_node.alternate {
        let else_result =
            walk_branch_for_all_paths(alt, else_paths, decisions, source, enhanced);
        terminated.extend(else_result.terminated);
        continued.extend(else_result.continued);
    } else {
        continued.extend(else_paths);
    }

    (terminated, continued)
}

/// Walk a branch statement (block or single statement) with multiple paths together.
fn walk_branch_for_all_paths<'a>(
    stmt: &'a Statement<'a>,
    paths: Vec<PathState>,
    decisions: &mut Vec<DecisionPoint>,
    source: &str,
    enhanced: bool,
) -> WalkResult {
    match stmt {
        Statement::BlockStatement(block) => {
            walk_statements(&block.body, paths, decisions, source, enhanced)
        }
        Statement::IfStatement(if_stmt) => {
            let (term, cont) =
                process_if_for_all_paths(if_stmt, paths, decisions, source, enhanced);
            WalkResult { terminated: term, continued: cont }
        }
        _ => {
            let mut terminated = Vec::new();
            let mut continued = Vec::new();
            for path in paths {
                let result = walk_simple_statement(stmt, path, source);
                terminated.extend(result.terminated);
                continued.extend(result.continued);
            }
            WalkResult { terminated, continued }
        }
    }
}

// --- O6c: Short-circuit expansion helpers ---

/// Returns true only for `&&` and `||` — boolean logical operators that can be
/// expanded into short-circuit rows. `??` (Coalesce) is excluded because it has
/// nullish semantics (not boolean truthiness) and is treated as an opaque atom.
fn is_boolean_logical(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::LogicalExpression(l)
            if matches!(l.operator, LogicalOperator::And | LogicalOperator::Or)
    )
}

/// Collect all atomic predicates from a `&&`/`||` expression tree (DFS left-to-right),
/// registering each as a new DecisionPoint.
/// `??` sub-expressions are treated as atoms (not recursed into).
fn collect_atomics<'a>(
    expr: &'a Expression<'a>,
    source: &str,
    decisions: &mut Vec<DecisionPoint>,
) {
    match expr {
        Expression::LogicalExpression(logical)
            if matches!(logical.operator, LogicalOperator::And | LogicalOperator::Or) =>
        {
            collect_atomics(&logical.left, source, decisions);
            collect_atomics(&logical.right, source, decisions);
        }
        _ => {
            let index = decisions.len();
            let span = expr.span();
            let pred_text = &source[span.start as usize..span.end as usize];
            let line = span_line(source, span.start);
            decisions.push(DecisionPoint {
                index,
                predicate: compact(pred_text),
                line,
            });
        }
    }
}

/// Count atomic predicates (leaves) in a `&&`/`||` expression tree.
/// `??` sub-expressions count as 1 atom.
fn count_atomics(expr: &Expression<'_>) -> usize {
    match expr {
        Expression::LogicalExpression(logical)
            if matches!(logical.operator, LogicalOperator::And | LogicalOperator::Or) =>
        {
            count_atomics(&logical.left) + count_atomics(&logical.right)
        }
        _ => 1,
    }
}

/// Generate short-circuit evaluation paths for a `&&`/`||` expression.
/// `start_idx`: index into `decisions` where this subexpression's atomics begin.
/// Returns list of (path_assignments, is_then_branch).
fn gen_paths(
    expr: &Expression<'_>,
    decisions: &[DecisionPoint],
    start_idx: usize,
    path: PathState,
) -> Vec<(PathState, bool)> {
    match expr {
        Expression::LogicalExpression(logical)
            if matches!(logical.operator, LogicalOperator::And | LogicalOperator::Or) =>
        {
            let left_count = count_atomics(&logical.left);
            let right_start = start_idx + left_count;
            let left_paths = gen_paths(&logical.left, decisions, start_idx, path);

            let mut results = Vec::new();
            for (left_path, left_true) in left_paths {
                match logical.operator {
                    LogicalOperator::And => {
                        if !left_true {
                            // Short-circuit: right not evaluated → whole expr is false
                            results.push((left_path, false));
                        } else {
                            // Right side determines the outcome
                            let right_paths =
                                gen_paths(&logical.right, decisions, right_start, left_path);
                            results.extend(right_paths);
                        }
                    }
                    LogicalOperator::Or => {
                        if left_true {
                            // Short-circuit: right not evaluated → whole expr is true
                            results.push((left_path, true));
                        } else {
                            // Right side determines the outcome
                            let right_paths =
                                gen_paths(&logical.right, decisions, right_start, left_path);
                            results.extend(right_paths);
                        }
                    }
                    _ => unreachable!("Coalesce filtered by is_boolean_logical"),
                }
            }
            results
        }
        _ => {
            // Atom (including ?? expressions): use the pre-registered decision at start_idx
            let index = decisions[start_idx].index;
            let mut then_path = path.clone();
            then_path.insert(index, "T");
            let mut else_path = path;
            else_path.insert(index, "F");
            vec![(then_path, true), (else_path, false)]
        }
    }
}

// --- Row construction ---

fn find_success_line(outcomes: &[PathOutcome]) -> Option<u32> {
    outcomes
        .iter()
        .filter(|o| o.outcome_kind == "return")
        .map(|o| o.line)
        .max()
}

fn to_truth_row(
    path: &PathOutcome,
    size: usize,
    success_line: Option<u32>,
    #[cfg(feature = "cfg-analysis")]
    cfg_ctx: Option<&crate::control_flow::context::CfgContext<'_>>,
) -> TruthRow {
    let mut values: Vec<String> = (0..size).map(|_| "*".to_string()).collect();
    for (index, value) in &path.assignments {
        if *index < size {
            values[*index] = value.to_string();
        }
    }

    let is_happy = success_line
        .map(|line| path.outcome_kind == "return" && path.line == line)
        .unwrap_or(false);

    let outcome_kind = if is_happy {
        "happy".to_string()
    } else {
        path.outcome_kind.clone()
    };

    let outcome_label = summarize_outcome(&path.outcome, is_happy);

    #[cfg(feature = "cfg-analysis")]
    let terminal_reachable = match (cfg_ctx, path.terminal_span) {
        (Some(ctx), Some(span)) => {
            crate::control_flow::reachability::is_span_unreachable(ctx, span)
                .map(|unreachable| !unreachable)
        }
        _ => None,
    };
    #[cfg(not(feature = "cfg-analysis"))]
    let terminal_reachable: Option<bool> = None;

    TruthRow {
        values,
        outcome: path.outcome.clone(),
        outcome_label,
        outcome_kind,
        line: path.line,
        terminal_reachable,
    }
}

fn dedupe_rows(rows: &mut Vec<TruthRow>) {
    let mut seen = std::collections::HashSet::new();
    rows.retain(|row| {
        let key = format!("{}:{}:{}", row.values.join(""), row.line, row.outcome);
        seen.insert(key)
    });
}

fn summarize_outcome(text: &str, is_happy: bool) -> String {
    if is_happy {
        return "Success".to_string();
    }

    let patterns = [
        r#"error\s*:\s*'([^']+)'"#,
        r#"error\s*:\s*"([^"]+)""#,
        r#"reason\s*:\s*'([^']+)'"#,
        r#"reason\s*:\s*"([^"]+)""#,
    ];

    for pattern in &patterns {
        if let Some(m) = simple_regex_match(text, pattern) {
            let label = to_pascal_case(&m);
            if !label.is_empty() {
                return label;
            }
        }
    }

    if text.starts_with("throw ") {
        return "Throw".to_string();
    }
    if text.contains("ok: false") {
        return "Failure".to_string();
    }
    if text.contains("ok: true") {
        return "Success".to_string();
    }
    "Outcome".to_string()
}

fn simple_regex_match(text: &str, pattern: &str) -> Option<String> {
    let keyword = pattern.split(r"\s*").next()?.split("\\b").last()?;
    let keyword = keyword.trim_matches(|c: char| !c.is_alphabetic());

    let lower_text = text.to_lowercase();
    let lower_keyword = keyword.to_lowercase();
    let pos = lower_text.find(&lower_keyword)?;
    let rest = &text[pos + keyword.len()..];

    let rest = rest.trim_start_matches(|c: char| c == ' ' || c == ':' || c == '\t');

    if rest.starts_with('\'') {
        let inner = &rest[1..];
        inner.find('\'').map(|end| inner[..end].to_string())
    } else if rest.starts_with('"') {
        let inner = &rest[1..];
        inner.find('"').map(|end| inner[..end].to_string())
    } else {
        None
    }
}

fn to_pascal_case(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn span_line(source: &str, start: u32) -> u32 {
    source[..start as usize].bytes().filter(|b| *b == b'\n').count() as u32 + 1
}
