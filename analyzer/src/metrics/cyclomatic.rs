use oxc_ast::ast::*;

pub fn analyze_cyclomatic_complexity(stmts: &[Statement<'_>]) -> u32 {
    let mut cc = 1u32;
    for stmt in stmts {
        walk_stmt(stmt, &mut cc);
    }
    cc
}

fn walk_stmt(stmt: &Statement<'_>, cc: &mut u32) {
    match stmt {
        Statement::IfStatement(if_stmt) => {
            *cc += 1;
            walk_expr(&if_stmt.test, cc);
            walk_stmt(&if_stmt.consequent, cc);
            if let Some(alt) = &if_stmt.alternate {
                walk_stmt(alt, cc);
            }
        }
        Statement::SwitchStatement(sw) => {
            for case in &sw.cases {
                // Count non-default cases (default has test == None)
                if case.test.is_some() {
                    *cc += 1;
                }
                for s in &case.consequent {
                    walk_stmt(s, cc);
                }
            }
        }
        Statement::WhileStatement(w) => {
            walk_stmt(&w.body, cc);
        }
        Statement::DoWhileStatement(dw) => {
            walk_stmt(&dw.body, cc);
        }
        Statement::ForStatement(f) => {
            walk_stmt(&f.body, cc);
        }
        Statement::ForInStatement(f) => {
            walk_stmt(&f.body, cc);
        }
        Statement::ForOfStatement(f) => {
            walk_stmt(&f.body, cc);
        }
        Statement::TryStatement(t) => {
            for s in &t.block.body {
                walk_stmt(s, cc);
            }
            if let Some(handler) = &t.handler {
                for s in &handler.body.body {
                    walk_stmt(s, cc);
                }
            }
            if let Some(fin) = &t.finalizer {
                for s in &fin.body {
                    walk_stmt(s, cc);
                }
            }
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                walk_stmt(s, cc);
            }
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                walk_expr(arg, cc);
            }
        }
        Statement::ExpressionStatement(expr_stmt) => {
            walk_expr(&expr_stmt.expression, cc);
        }
        Statement::VariableDeclaration(var_decl) => {
            for decl in &var_decl.declarations {
                if let Some(init) = &decl.init {
                    walk_expr_skip_fn(init, cc);
                }
            }
        }
        // Don't recurse into nested functions
        Statement::FunctionDeclaration(_) => {}
        _ => {}
    }
}

fn walk_expr(expr: &Expression<'_>, cc: &mut u32) {
    match expr {
        Expression::ConditionalExpression(cond) => {
            *cc += 1;
            walk_expr(&cond.test, cc);
            walk_expr(&cond.consequent, cc);
            walk_expr(&cond.alternate, cc);
        }
        Expression::LogicalExpression(log) => {
            walk_expr(&log.left, cc);
            walk_expr(&log.right, cc);
        }
        Expression::BinaryExpression(bin) => {
            walk_expr(&bin.left, cc);
            walk_expr(&bin.right, cc);
        }
        Expression::UnaryExpression(u) => {
            walk_expr(&u.argument, cc);
        }
        Expression::CallExpression(call) => {
            walk_expr_skip_fn(&call.callee, cc);
        }
        Expression::AwaitExpression(aw) => {
            walk_expr(&aw.argument, cc);
        }
        Expression::TSNonNullExpression(inner) => walk_expr(&inner.expression, cc),
        Expression::TSAsExpression(inner) => walk_expr(&inner.expression, cc),
        Expression::ParenthesizedExpression(inner) => walk_expr(&inner.expression, cc),
        // Don't recurse into function expressions
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
        _ => {}
    }
}

fn walk_expr_skip_fn(expr: &Expression<'_>, cc: &mut u32) {
    walk_expr(expr, cc);
}
