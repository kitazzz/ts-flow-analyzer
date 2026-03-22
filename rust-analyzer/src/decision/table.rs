use super::mcdc::build_mcdc_cases;
use crate::ast::collect_functions::FunctionNode;
use crate::config::DecisionTableConfig;
use crate::model::{DecisionPoint, DecisionTableData, TruthRow};
use oxc_ast::ast::*;
use oxc_span::{GetSpan, Span};
use regex::Regex;
use std::collections::HashMap;

pub fn build_decision_table<'a>(
    node: &FunctionNode<'a>,
    symbol_name: &str,
    symbol_kind: &str,
    source: &str,
    config: &DecisionTableConfig,
    enhanced: bool,
    #[cfg(feature = "cfg-analysis")] cfg_ctx: Option<&crate::control_flow::context::CfgContext<'_>>,
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

    let success_line = find_success_line(&walk.terminated, config);
    let mut truth_rows: Vec<TruthRow> = walk
        .terminated
        .iter()
        .map(|path| {
            to_truth_row(
                path,
                decisions.len(),
                success_line,
                config,
                #[cfg(feature = "cfg-analysis")]
                cfg_ctx,
            )
        })
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
        FunctionNode::Function(f) => f.body.as_ref().map(|b| b.statements.as_slice()),
        FunctionNode::Arrow(arrow) => Some(arrow.body.statements.as_slice()),
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
                let result = walk_statements(&block.body, continued, decisions, source, enhanced);
                terminated.extend(result.terminated);
                continued = result.continued;
            }
            Statement::TryStatement(try_stmt) => {
                let has_catch = try_stmt.handler.is_some();

                let try_input = if has_catch {
                    continued.clone() // need original paths for implicit-exception fallback
                } else {
                    std::mem::take(&mut continued)
                };

                let try_result =
                    walk_statements(&try_stmt.block.body, try_input, decisions, source, enhanced);

                // Separate try terminations: returns are true terminals, throws are catch candidates
                let (try_returns, try_throws): (Vec<_>, Vec<_>) = try_result
                    .terminated
                    .into_iter()
                    .partition(|p| p.outcome_kind != "throw");

                let mut all_terminated = try_returns;
                let mut all_continued = try_result.continued;

                if let Some(handler) = &try_stmt.handler {
                    // catch input = explicit throw paths + pre-try paths (implicit exceptions).
                    // Any statement in try can throw at runtime, so pre-try paths
                    // are always included as implicit exception sources.
                    // Explicit throws additionally carry their accumulated decision
                    // assignments for more precise rows.
                    let mut catch_input: Vec<PathState> = continued;
                    catch_input.extend(try_throws.into_iter().map(|p| p.assignments));

                    let catch_result = walk_statements(
                        &handler.body.body,
                        catch_input,
                        decisions,
                        source,
                        enhanced,
                    );
                    all_terminated.extend(catch_result.terminated);
                    all_continued.extend(catch_result.continued);
                } else {
                    // No catch: throws remain terminated
                    all_terminated.extend(try_throws);
                }

                // Walk finally block if present.
                // finally always executes regardless of how try/catch ended.
                //
                // Design: walk finally ONCE with ALL paths (continued + terminated)
                // merged together so decisions are registered exactly once.
                // To distinguish origins afterward, we tag each path with a
                // reserved key encoding its origin index.
                if let Some(finalizer) = &try_stmt.finalizer {
                    let pre_fin_terminated = std::mem::take(&mut all_terminated);
                    let cont_count = all_continued.len();

                    // Reserved key that won't collide with real decision indices.
                    let tag_key: usize = usize::MAX;

                    let mut fin_input: Vec<PathState> =
                        Vec::with_capacity(cont_count + pre_fin_terminated.len());
                    // Tag continued-origin paths with "C"
                    for mut p in all_continued {
                        p.insert(tag_key, "C");
                        fin_input.push(p);
                    }
                    // Tag terminated-origin paths with "T" and store their index
                    // in a second reserved key so we can recover the exact original.
                    let idx_key: usize = usize::MAX - 1;
                    for (i, orig) in pre_fin_terminated.iter().enumerate() {
                        let mut p = orig.assignments.clone();
                        p.insert(tag_key, "T");
                        // Encode index as one of a fixed set of static strings
                        p.insert(idx_key, leak_index(i));
                        fin_input.push(p);
                    }

                    let fin_result =
                        walk_statements(&finalizer.body, fin_input, decisions, source, enhanced);

                    all_terminated = Vec::new();
                    all_continued = Vec::new();

                    // Process terminated outputs from finally
                    for mut out in fin_result.terminated {
                        let origin = out.assignments.remove(&tag_key);
                        out.assignments.remove(&idx_key);
                        match origin {
                            Some("C") => {
                                // Continued-origin path terminated in finally → new terminal
                                all_terminated.push(out);
                            }
                            Some("T") => {
                                // Terminated-origin path: finally overrides the original outcome
                                all_terminated.push(out);
                            }
                            _ => {
                                all_terminated.push(out);
                            }
                        }
                    }

                    // Process continued outputs from finally (fell through)
                    for mut cont_path in fin_result.continued {
                        let origin = cont_path.remove(&tag_key);
                        let idx_str = cont_path.remove(&idx_key);
                        match origin {
                            Some("C") => {
                                // Continued-origin path still continues past try
                                all_continued.push(cont_path);
                            }
                            Some("T") => {
                                // Terminated-origin path fell through finally →
                                // original outcome is preserved
                                let term_idx = parse_index(idx_str);
                                if let Some(orig) = term_idx.and_then(|i| pre_fin_terminated.get(i))
                                {
                                    all_terminated.push(orig.clone());
                                }
                            }
                            _ => {
                                all_continued.push(cont_path);
                            }
                        }
                    }
                }

                terminated.extend(all_terminated);
                continued = all_continued;
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

    WalkResult {
        terminated,
        continued,
    }
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
        let test_span = if_node.test.span();
        let pred_text =
            &source[test_span.start as usize..test_span.end as usize];
        let line = span_line(source, if_node.span.start);
        decisions.push(DecisionPoint {
            index,
            predicate: compact(pred_text),
            line,
            span_start: Some(test_span.start),
            span_end: Some(test_span.end),
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
        let else_result = walk_branch_for_all_paths(alt, else_paths, decisions, source, enhanced);
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
            WalkResult {
                terminated: term,
                continued: cont,
            }
        }
        Statement::TryStatement(_) => walk_statements(
            std::slice::from_ref(stmt),
            paths,
            decisions,
            source,
            enhanced,
        ),
        _ => {
            let mut terminated = Vec::new();
            let mut continued = Vec::new();
            for path in paths {
                let result = walk_simple_statement(stmt, path, source);
                terminated.extend(result.terminated);
                continued.extend(result.continued);
            }
            WalkResult {
                terminated,
                continued,
            }
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
fn collect_atomics<'a>(expr: &'a Expression<'a>, source: &str, decisions: &mut Vec<DecisionPoint>) {
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
                span_start: Some(span.start),
                span_end: Some(span.end),
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

/// Detect whether an outcome text contains a positive boolean property like
/// `ok: true`, `allowed: true`, `success: true`, etc.
fn has_positive_bool(text: &str, config: &DecisionTableConfig) -> bool {
    config
        .success_when_true
        .iter()
        .any(|field| field_has_bool(text, field, true))
}

/// Detect whether an outcome text contains a negative boolean property like
/// `ok: false`, `allowed: false`, etc.
fn has_negative_bool(text: &str, config: &DecisionTableConfig) -> bool {
    config
        .failure_when_false
        .iter()
        .any(|field| field_has_bool(text, field, false))
        || config
            .failure_when_present
            .iter()
            .any(|field| field_is_present(text, field))
}

fn find_success_line(outcomes: &[PathOutcome], config: &DecisionTableConfig) -> Option<u32> {
    // Priority 1: return with a positive boolean property (e.g. ok: true, allowed: true)
    let positive = outcomes
        .iter()
        .filter(|o| o.outcome_kind == "return" && has_positive_bool(&o.outcome, config))
        .map(|o| o.line)
        .max();
    if positive.is_some() {
        return positive;
    }

    // Priority 2: return without any negative boolean property and not a throw (max line)
    let non_failure = outcomes
        .iter()
        .filter(|o| o.outcome_kind == "return" && !has_negative_bool(&o.outcome, config))
        .map(|o| o.line)
        .max();
    // Only use non_failure if such returns actually exist; if every return is
    // a failure (e.g. all paths end in ok: false), there is no success line.
    non_failure
}

fn to_truth_row(
    path: &PathOutcome,
    size: usize,
    success_line: Option<u32>,
    config: &DecisionTableConfig,
    #[cfg(feature = "cfg-analysis")] cfg_ctx: Option<&crate::control_flow::context::CfgContext<'_>>,
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

    let outcome_label = summarize_outcome(&path.outcome, is_happy, config);

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

fn summarize_outcome(text: &str, is_happy: bool, config: &DecisionTableConfig) -> String {
    if is_happy {
        return "Success".to_string();
    }

    for field in &config.failure_when_present {
        if let Some(m) = extract_string_field(text, field) {
            let label = to_pascal_case(&m);
            if !label.is_empty() {
                return label;
            }
        }
    }

    if text.starts_with("throw ") {
        return "Throw".to_string();
    }
    if has_negative_bool(text, config) {
        return "Failure".to_string();
    }
    if has_positive_bool(text, config) {
        return "Success".to_string();
    }
    "Outcome".to_string()
}

fn field_has_bool(text: &str, field: &str, expected: bool) -> bool {
    let escaped = regex::escape(field);
    let bool_text = if expected { "true" } else { "false" };
    let pattern = format!(r#"(?i)\b['"]?{}['"]?\b\s*:\s*{}\b"#, escaped, bool_text);
    Regex::new(&pattern)
        .map(|regex| regex.is_match(text))
        .unwrap_or(false)
}

fn field_is_present(text: &str, field: &str) -> bool {
    let escaped = regex::escape(field);
    let pattern = format!(r#"(?i)\b['"]?{}['"]?\b\s*:"#, escaped);
    Regex::new(&pattern)
        .map(|regex| regex.is_match(text))
        .unwrap_or(false)
}

fn extract_string_field(text: &str, field: &str) -> Option<String> {
    let lower_text = text.to_lowercase();
    let lower_field = field.to_lowercase();
    let pos = lower_text.find(&lower_field)?;
    let rest = &text[pos + field.len()..];

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
    source[..start as usize]
        .bytes()
        .filter(|b| *b == b'\n')
        .count() as u32
        + 1
}

/// Encode an index as a &'static str for use in PathState tags.
/// We leak a small number of strings (one per unique index used in a single finally).
fn leak_index(i: usize) -> &'static str {
    // For small indices, use pre-allocated strings to avoid leaking
    const PREALLOC: &[&str] = &[
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
        "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31",
    ];
    if i < PREALLOC.len() {
        PREALLOC[i]
    } else {
        Box::leak(i.to_string().into_boxed_str())
    }
}

fn parse_index(s: Option<&'static str>) -> Option<usize> {
    s.and_then(|s| s.parse().ok())
}
