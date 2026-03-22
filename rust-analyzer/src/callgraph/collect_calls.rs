use oxc_ast::ast::*;

use super::model::{CallKind, CallSite};

pub fn collect_calls(stmts: &[Statement<'_>], source: &str) -> Vec<CallSite> {
    let mut results = Vec::new();
    for stmt in stmts {
        collect_from_stmt(stmt, source, &mut results);
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

fn source_text(source: &str, start: u32, end: u32) -> String {
    source[start as usize..end as usize].trim().to_string()
}

fn collect_from_stmt(stmt: &Statement<'_>, source: &str, out: &mut Vec<CallSite>) {
    match stmt {
        Statement::ExpressionStatement(expr_stmt) => {
            collect_from_expr(&expr_stmt.expression, source, out);
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                collect_from_expr(arg, source, out);
            }
        }
        Statement::ThrowStatement(thr) => {
            collect_from_expr(&thr.argument, source, out);
        }
        Statement::VariableDeclaration(var_decl) => {
            for declarator in &var_decl.declarations {
                if let Some(init) = &declarator.init {
                    collect_from_expr(init, source, out);
                }
            }
        }
        Statement::IfStatement(s) => {
            collect_from_expr(&s.test, source, out);
            collect_from_stmt(&s.consequent, source, out);
            if let Some(alt) = &s.alternate {
                collect_from_stmt(alt, source, out);
            }
        }
        Statement::BlockStatement(b) => {
            for s in &b.body {
                collect_from_stmt(s, source, out);
            }
        }
        Statement::WhileStatement(w) => {
            collect_from_expr(&w.test, source, out);
            collect_from_stmt(&w.body, source, out);
        }
        Statement::DoWhileStatement(dw) => {
            collect_from_expr(&dw.test, source, out);
            collect_from_stmt(&dw.body, source, out);
        }
        Statement::ForStatement(f) => {
            if let Some(test) = &f.test {
                collect_from_expr(test, source, out);
            }
            collect_from_stmt(&f.body, source, out);
        }
        Statement::ForInStatement(f) => collect_from_stmt(&f.body, source, out),
        Statement::ForOfStatement(f) => collect_from_stmt(&f.body, source, out),
        Statement::SwitchStatement(sw) => {
            collect_from_expr(&sw.discriminant, source, out);
            for case in &sw.cases {
                if let Some(test) = &case.test {
                    collect_from_expr(test, source, out);
                }
                for s in &case.consequent {
                    collect_from_stmt(s, source, out);
                }
            }
        }
        Statement::TryStatement(t) => {
            for s in &t.block.body {
                collect_from_stmt(s, source, out);
            }
            if let Some(handler) = &t.handler {
                for s in &handler.body.body {
                    collect_from_stmt(s, source, out);
                }
            }
            if let Some(fin) = &t.finalizer {
                for s in &fin.body {
                    collect_from_stmt(s, source, out);
                }
            }
        }
        // Don't recurse into nested function declarations
        Statement::FunctionDeclaration(_) => {}
        _ => {}
    }
}

fn collect_from_expr(expr: &Expression<'_>, source: &str, out: &mut Vec<CallSite>) {
    match expr {
        Expression::CallExpression(call) => {
            let site = classify_call(&call.callee, source, call.span.start, call.span.end);
            out.push(site);
            // Also collect calls within arguments
            for arg in &call.arguments {
                match arg {
                    Argument::SpreadElement(spread) => {
                        collect_from_expr(&spread.argument, source, out);
                    }
                    _ => {
                        collect_from_expr(arg.to_expression(), source, out);
                    }
                }
            }
        }
        Expression::AwaitExpression(await_expr) => {
            collect_from_expr(&await_expr.argument, source, out);
        }
        Expression::AssignmentExpression(assign) => {
            collect_from_expr(&assign.right, source, out);
        }
        Expression::ConditionalExpression(cond) => {
            collect_from_expr(&cond.test, source, out);
            collect_from_expr(&cond.consequent, source, out);
            collect_from_expr(&cond.alternate, source, out);
        }
        Expression::LogicalExpression(logical) => {
            collect_from_expr(&logical.left, source, out);
            collect_from_expr(&logical.right, source, out);
        }
        Expression::UnaryExpression(unary) => {
            collect_from_expr(&unary.argument, source, out);
        }
        Expression::BinaryExpression(bin) => {
            collect_from_expr(&bin.left, source, out);
            collect_from_expr(&bin.right, source, out);
        }
        Expression::TemplateLiteral(tmpl) => {
            for expr in &tmpl.expressions {
                collect_from_expr(expr, source, out);
            }
        }
        Expression::TaggedTemplateExpression(tagged) => {
            collect_from_expr(&tagged.tag, source, out);
        }
        Expression::NewExpression(new_expr) => {
            for arg in &new_expr.arguments {
                match arg {
                    Argument::SpreadElement(spread) => {
                        collect_from_expr(&spread.argument, source, out);
                    }
                    _ => {
                        collect_from_expr(arg.to_expression(), source, out);
                    }
                }
            }
        }
        Expression::SequenceExpression(seq) => {
            for expr in &seq.expressions {
                collect_from_expr(expr, source, out);
            }
        }
        Expression::ParenthesizedExpression(paren) => {
            collect_from_expr(&paren.expression, source, out);
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                match prop {
                    ObjectPropertyKind::ObjectProperty(p) => {
                        collect_from_expr(&p.value, source, out);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_from_expr(&spread.argument, source, out);
                    }
                }
            }
        }
        Expression::ArrayExpression(arr) => {
            for elem in &arr.elements {
                match elem {
                    ArrayExpressionElement::SpreadElement(spread) => {
                        collect_from_expr(&spread.argument, source, out);
                    }
                    ArrayExpressionElement::Elision(_) => {}
                    _ => {
                        collect_from_expr(elem.to_expression(), source, out);
                    }
                }
            }
        }
        // Don't recurse into nested arrow/function expressions
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {}
        _ => {}
    }
}

fn classify_call(
    callee: &Expression<'_>,
    source: &str,
    call_start: u32,
    call_end: u32,
) -> CallSite {
    let callee_text = source_text(source, call_start, call_end);
    let line = span_line(source, call_start);

    match callee {
        // foo()
        Expression::Identifier(id) => CallSite {
            kind: CallKind::Direct,
            callee_text,
            target_name: id.name.to_string(),
            receiver: None,
            line,
            span_start: call_start,
            span_end: call_end,
        },
        // obj.method() or this.method() or this.repo.method()
        Expression::StaticMemberExpression(member) => {
            let property = member.property.name.to_string();
            let receiver_text =
                source_text(source, member.object.span().start, member.object.span().end);
            if receiver_text == "this" {
                CallSite {
                    kind: CallKind::ThisMethod,
                    callee_text,
                    target_name: property,
                    receiver: Some("this".to_string()),
                    line,
                    span_start: call_start,
                    span_end: call_end,
                }
            } else if receiver_text == "super" {
                CallSite {
                    kind: CallKind::Super,
                    callee_text,
                    target_name: property,
                    receiver: Some("super".to_string()),
                    line,
                    span_start: call_start,
                    span_end: call_end,
                }
            } else {
                CallSite {
                    kind: CallKind::MemberCall,
                    callee_text,
                    target_name: property,
                    receiver: Some(receiver_text),
                    line,
                    span_start: call_start,
                    span_end: call_end,
                }
            }
        }
        // obj[expr]() or other dynamic patterns
        Expression::ComputedMemberExpression(member) => {
            let receiver_text =
                source_text(source, member.object.span().start, member.object.span().end);
            CallSite {
                kind: CallKind::MemberCall,
                callee_text,
                target_name: "<dynamic>".to_string(),
                receiver: Some(receiver_text),
                line,
                span_start: call_start,
                span_end: call_end,
            }
        }
        _ => CallSite {
            kind: CallKind::MemberCall,
            callee_text,
            target_name: "<dynamic>".to_string(),
            receiver: None,
            line,
            span_start: call_start,
            span_end: call_end,
        },
    }
}

use oxc_span::GetSpan;
