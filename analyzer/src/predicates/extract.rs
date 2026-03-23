use super::classify::classify_predicate;
use crate::model::{AtomicPredicate, DecisionContext};
use oxc_ast::ast::*;
use oxc_span::GetSpan;

pub fn extract_predicates(stmts: &[Statement<'_>], source: &str) -> Vec<AtomicPredicate> {
    let mut results = Vec::new();
    for stmt in stmts {
        extract_from_stmt(stmt, source, &mut results, false);
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

fn extract_from_stmt(
    stmt: &Statement<'_>,
    source: &str,
    out: &mut Vec<AtomicPredicate>,
    _skip_nested: bool,
) {
    match stmt {
        Statement::IfStatement(if_stmt) => {
            // Determine context: is this an else-if? We handle that when recursing
            collect_from_expr(&if_stmt.test, DecisionContext::If, false, source, out);
            extract_from_stmt(&if_stmt.consequent, source, out, false);
            if let Some(alt) = &if_stmt.alternate {
                match alt {
                    Statement::IfStatement(nested_if) => {
                        // else-if: collect with ElseIf context
                        collect_from_expr(
                            &nested_if.test,
                            DecisionContext::ElseIf,
                            false,
                            source,
                            out,
                        );
                        extract_from_stmt(&nested_if.consequent, source, out, false);
                        if let Some(nested_alt) = &nested_if.alternate {
                            extract_from_stmt(nested_alt, source, out, false);
                        }
                    }
                    other => {
                        extract_from_stmt(other, source, out, false);
                    }
                }
            }
        }
        Statement::WhileStatement(w) => {
            collect_from_expr(&w.test, DecisionContext::While, false, source, out);
            extract_from_stmt(&w.body, source, out, false);
        }
        Statement::DoWhileStatement(dw) => {
            collect_from_expr(&dw.test, DecisionContext::DoWhile, false, source, out);
            extract_from_stmt(&dw.body, source, out, false);
        }
        Statement::ForStatement(f) => {
            if let Some(test) = &f.test {
                collect_from_expr(test, DecisionContext::For, false, source, out);
            }
            extract_from_stmt(&f.body, source, out, false);
        }
        Statement::ForInStatement(f) => {
            extract_from_stmt(&f.body, source, out, false);
        }
        Statement::ForOfStatement(f) => {
            extract_from_stmt(&f.body, source, out, false);
        }
        Statement::SwitchStatement(sw) => {
            for case in &sw.cases {
                if let Some(test) = &case.test {
                    collect_from_expr(test, DecisionContext::Case, false, source, out);
                }
                for s in &case.consequent {
                    extract_from_stmt(s, source, out, false);
                }
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                extract_from_stmt(s, source, out, false);
            }
        }
        Statement::TryStatement(t) => {
            for s in &t.block.body {
                extract_from_stmt(s, source, out, false);
            }
            if let Some(handler) = &t.handler {
                for s in &handler.body.body {
                    extract_from_stmt(s, source, out, false);
                }
            }
            if let Some(fin) = &t.finalizer {
                for s in &fin.body {
                    extract_from_stmt(s, source, out, false);
                }
            }
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                extract_conditionals_from_expr(arg, source, out);
            }
        }
        Statement::ExpressionStatement(expr_stmt) => {
            extract_conditionals_from_expr(&expr_stmt.expression, source, out);
        }
        Statement::VariableDeclaration(var_decl) => {
            for decl in &var_decl.declarations {
                if let Some(init) = &decl.init {
                    extract_conditionals_from_expr(init, source, out);
                }
            }
        }
        // Don't recurse into nested functions
        Statement::FunctionDeclaration(_) => {}
        _ => {}
    }
}

/// Extract ternary conditions from expression (not function bodies)
fn extract_conditionals_from_expr(
    expr: &Expression<'_>,
    source: &str,
    out: &mut Vec<AtomicPredicate>,
) {
    match expr {
        Expression::ConditionalExpression(cond) => {
            collect_from_expr(&cond.test, DecisionContext::Ternary, false, source, out);
            extract_conditionals_from_expr(&cond.consequent, source, out);
            extract_conditionals_from_expr(&cond.alternate, source, out);
        }
        Expression::LogicalExpression(log) => {
            extract_conditionals_from_expr(&log.left, source, out);
            extract_conditionals_from_expr(&log.right, source, out);
        }
        Expression::CallExpression(call) => {
            extract_conditionals_from_expr(&call.callee, source, out);
            for arg in &call.arguments {
                match arg {
                    Argument::SpreadElement(spread) => {
                        extract_conditionals_from_expr(&spread.argument, source, out)
                    }
                    e => extract_conditionals_from_expr(e.to_expression(), source, out),
                }
            }
        }
        Expression::AwaitExpression(aw) => {
            extract_conditionals_from_expr(&aw.argument, source, out)
        }
        Expression::TSNonNullExpression(inner) => {
            extract_conditionals_from_expr(&inner.expression, source, out)
        }
        Expression::TSAsExpression(inner) => {
            extract_conditionals_from_expr(&inner.expression, source, out)
        }
        Expression::ParenthesizedExpression(inner) => {
            extract_conditionals_from_expr(&inner.expression, source, out)
        }
        // Don't recurse into function bodies
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
        _ => {}
    }
}

fn collect_from_expr(
    expr: &Expression<'_>,
    ctx: DecisionContext,
    negated: bool,
    source: &str,
    out: &mut Vec<AtomicPredicate>,
) {
    match expr {
        // Split logical AND/OR/coalesce into atomic predicates
        Expression::LogicalExpression(log) => {
            collect_from_expr(&log.left, ctx.clone(), negated, source, out);
            collect_from_expr(&log.right, ctx, negated, source, out);
        }
        // Unwrap parentheses
        Expression::ParenthesizedExpression(inner) => {
            collect_from_expr(&inner.expression, ctx, negated, source, out);
        }
        // Flip negation
        Expression::UnaryExpression(u) if matches!(u.operator, UnaryOperator::LogicalNot) => {
            collect_from_expr(&u.argument, ctx, !negated, source, out);
        }
        Expression::TSNonNullExpression(inner) => {
            collect_from_expr(&inner.expression, ctx, negated, source, out);
        }
        Expression::TSAsExpression(inner) => {
            collect_from_expr(&inner.expression, ctx, negated, source, out);
        }
        // Leaf - classify and record
        _ => {
            let text = source[expr.span().start as usize..expr.span().end as usize]
                .trim()
                .to_string();
            let line = span_line(source, expr.span().start);
            let kind = classify_predicate(expr, source);
            out.push(AtomicPredicate {
                text,
                negated,
                kind,
                context: ctx,
                line,
                normalized_name: None,
                true_meaning: None,
                false_meaning: None,
            });
        }
    }
}
