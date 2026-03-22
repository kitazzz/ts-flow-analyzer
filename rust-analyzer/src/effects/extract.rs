use crate::model::{Effect, EffectKind, SideEffectClass};
use oxc_ast::ast::*;

pub fn extract_effects(stmts: &[Statement<'_>], source: &str) -> Vec<Effect> {
    let mut results = Vec::new();
    for stmt in stmts {
        extract_from_stmt(stmt, source, &mut results);
    }
    results
}

fn span_line(source: &str, start: u32) -> u32 {
    source[..start as usize]
        .bytes()
        .filter(|b| *b == b'\n')
        .count() as u32
        + 1
}

fn extract_from_stmt(stmt: &Statement<'_>, source: &str, out: &mut Vec<Effect>) {
    match stmt {
        Statement::ReturnStatement(ret) => {
            let text = source[ret.span.start as usize..ret.span.end as usize]
                .trim()
                .to_string();
            let line = span_line(source, ret.span.start);
            out.push(Effect {
                kind: EffectKind::Return,
                side_effect: SideEffectClass::Return,
                text,
                line,
            });
        }
        Statement::ThrowStatement(thr) => {
            let text = source[thr.span.start as usize..thr.span.end as usize]
                .trim()
                .to_string();
            let line = span_line(source, thr.span.start);
            out.push(Effect {
                kind: EffectKind::Throw,
                side_effect: SideEffectClass::Throw,
                text,
                line,
            });
        }
        Statement::ExpressionStatement(expr_stmt) => {
            let expr = &expr_stmt.expression;
            let text = source[expr_stmt.span.start as usize..expr_stmt.span.end as usize]
                .trim()
                .to_string();
            let line = span_line(source, expr_stmt.span.start);

            match expr {
                Expression::AssignmentExpression(assign) => {
                    if matches!(assign.operator, AssignmentOperator::Assign) {
                        out.push(Effect {
                            kind: EffectKind::Assignment,
                            side_effect: SideEffectClass::StateWrite,
                            text,
                            line,
                        });
                    }
                }
                Expression::CallExpression(_) | Expression::AwaitExpression(_) => {
                    out.push(Effect {
                        kind: EffectKind::Call,
                        side_effect: classify_call_side_effect(&text),
                        text,
                        line,
                    });
                }
                _ => {}
            }
        }
        Statement::VariableDeclaration(var_decl) => {
            let text = source[var_decl.span.start as usize..var_decl.span.end as usize]
                .trim()
                .to_string();
            let line = span_line(source, var_decl.span.start);
            let has_call = text.contains('(') && (text.contains("await ") || text.contains("= "));
            if has_call {
                out.push(Effect {
                    kind: EffectKind::Call,
                    side_effect: classify_call_side_effect(&text),
                    text,
                    line,
                });
            }
        }
        Statement::IfStatement(s) => {
            extract_from_stmt(&s.consequent, source, out);
            if let Some(alt) = &s.alternate {
                extract_from_stmt(alt, source, out);
            }
        }
        Statement::BlockStatement(b) => {
            for s in &b.body {
                extract_from_stmt(s, source, out);
            }
        }
        Statement::WhileStatement(w) => extract_from_stmt(&w.body, source, out),
        Statement::DoWhileStatement(dw) => extract_from_stmt(&dw.body, source, out),
        Statement::ForStatement(f) => extract_from_stmt(&f.body, source, out),
        Statement::ForInStatement(f) => extract_from_stmt(&f.body, source, out),
        Statement::ForOfStatement(f) => extract_from_stmt(&f.body, source, out),
        Statement::SwitchStatement(sw) => {
            for case in &sw.cases {
                for s in &case.consequent {
                    extract_from_stmt(s, source, out);
                }
            }
        }
        Statement::TryStatement(t) => {
            for s in &t.block.body {
                extract_from_stmt(s, source, out);
            }
            if let Some(handler) = &t.handler {
                for s in &handler.body.body {
                    extract_from_stmt(s, source, out);
                }
            }
            if let Some(fin) = &t.finalizer {
                for s in &fin.body {
                    extract_from_stmt(s, source, out);
                }
            }
        }
        // Don't recurse into nested functions
        Statement::FunctionDeclaration(_) => {}
        _ => {}
    }
}

fn classify_call_side_effect(text: &str) -> SideEffectClass {
    // Log pattern
    if is_match_log(text) {
        return SideEffectClass::Logging;
    }
    // DB write
    if is_match_db_write(text) {
        return SideEffectClass::DbWrite;
    }
    // DB read
    if is_match_db_read(text) {
        return SideEffectClass::DbRead;
    }
    // External API
    if is_match_external(text) {
        return SideEffectClass::ExternalApi;
    }
    SideEffectClass::PureCall
}

/// Check if `text` contains a word that starts with `keyword` (camelCase prefix match).
/// Matches: keyword( or keywordFoo (uppercase after) or keyword at end of identifier.
/// The match must be at a word boundary (not in the middle of an identifier).
fn matches_keyword(text: &str, keyword: &str) -> bool {
    let lower_text = text.to_lowercase();
    let keyword_lower = keyword.to_lowercase();
    let mut search_from = 0;
    while let Some(pos) = lower_text[search_from..].find(&keyword_lower) {
        let abs_pos = search_from + pos;
        // Check character before is a word boundary
        let before_ok = abs_pos == 0 || {
            let before_char = lower_text[..abs_pos].chars().last().unwrap_or(' ');
            !before_char.is_ascii_alphanumeric() && before_char != '_'
        };
        if before_ok {
            let after = &text[abs_pos + keyword.len()..];
            let after_char = after.chars().next();
            // Valid if followed by: '(' or uppercase (camelCase) or end of identifier
            let after_ok = match after_char {
                None => true,
                Some('(') => true,
                Some(c) if c.is_ascii_uppercase() => true,
                Some(c) if !c.is_ascii_alphanumeric() && c != '_' => true,
                _ => false,
            };
            if after_ok {
                return true;
            }
        }
        search_from = abs_pos + 1;
        if search_from >= lower_text.len() {
            break;
        }
    }
    false
}

fn is_match_log(text: &str) -> bool {
    for keyword in &["log", "warn", "debug", "console", "logger", "trace"] {
        if matches_keyword(text, keyword) {
            return true;
        }
    }
    false
}

fn is_match_db_write(text: &str) -> bool {
    for keyword in &[
        "save", "update", "delete", "remove", "insert", "upsert", "create", "put", "patch",
    ] {
        if matches_keyword(text, keyword) {
            return true;
        }
    }
    false
}

fn is_match_db_read(text: &str) -> bool {
    for keyword in &[
        "find", "get", "query", "fetch", "load", "select", "search", "list",
    ] {
        if matches_keyword(text, keyword) {
            return true;
        }
    }
    false
}

fn is_match_external(text: &str) -> bool {
    for keyword in &[
        "http", "axios", "request", "post", "send", "publish", "emit", "notify", "dispatch",
    ] {
        if matches_keyword(text, keyword) {
            return true;
        }
    }
    false
}
