use oxc_ast::ast::*;

pub fn analyze_max_nesting_depth(stmts: &[Statement<'_>]) -> u32 {
    let mut max = 0;
    for stmt in stmts {
        let d = walk_stmt_nesting(stmt, 0, true);
        if d > max {
            max = d;
        }
    }
    max
}

fn walk_stmt_nesting(stmt: &Statement<'_>, current_depth: u32, _is_root: bool) -> u32 {
    match stmt {
        Statement::IfStatement(if_stmt) => {
            let depth = current_depth + 1;
            let mut max = depth;
            let d = walk_stmt_nesting(&if_stmt.consequent, depth, false);
            if d > max { max = d; }
            if let Some(alt) = &if_stmt.alternate {
                let d = walk_stmt_nesting(alt, depth, false);
                if d > max { max = d; }
            }
            max
        }
        Statement::SwitchStatement(sw) => {
            let depth = current_depth + 1;
            let mut max = depth;
            for case in &sw.cases {
                for s in &case.consequent {
                    let d = walk_stmt_nesting(s, depth, false);
                    if d > max { max = d; }
                }
            }
            max
        }
        Statement::WhileStatement(w) => {
            let depth = current_depth + 1;
            let d = walk_stmt_nesting(&w.body, depth, false);
            d.max(depth)
        }
        Statement::DoWhileStatement(dw) => {
            let depth = current_depth + 1;
            let d = walk_stmt_nesting(&dw.body, depth, false);
            d.max(depth)
        }
        Statement::ForStatement(f) => {
            let depth = current_depth + 1;
            let d = walk_stmt_nesting(&f.body, depth, false);
            d.max(depth)
        }
        Statement::ForInStatement(f) => {
            let depth = current_depth + 1;
            let d = walk_stmt_nesting(&f.body, depth, false);
            d.max(depth)
        }
        Statement::ForOfStatement(f) => {
            let depth = current_depth + 1;
            let d = walk_stmt_nesting(&f.body, depth, false);
            d.max(depth)
        }
        Statement::TryStatement(t) => {
            let depth = current_depth + 1;
            let mut max = depth;
            for s in &t.block.body {
                let d = walk_stmt_nesting(s, depth, false);
                if d > max { max = d; }
            }
            if let Some(handler) = &t.handler {
                for s in &handler.body.body {
                    let d = walk_stmt_nesting(s, depth, false);
                    if d > max { max = d; }
                }
            }
            if let Some(fin) = &t.finalizer {
                for s in &fin.body {
                    let d = walk_stmt_nesting(s, depth, false);
                    if d > max { max = d; }
                }
            }
            max
        }
        Statement::BlockStatement(block) => {
            let mut max = current_depth;
            for s in &block.body {
                let d = walk_stmt_nesting(s, current_depth, false);
                if d > max { max = d; }
            }
            max
        }
        // Skip nested function declarations
        Statement::FunctionDeclaration(_) => current_depth,
        Statement::ExpressionStatement(expr_stmt) => {
            walk_expr_nesting(&expr_stmt.expression, current_depth)
        }
        Statement::VariableDeclaration(var_decl) => {
            let mut max = current_depth;
            for decl in &var_decl.declarations {
                if let Some(init) = &decl.init {
                    let d = walk_expr_nesting(init, current_depth);
                    if d > max { max = d; }
                }
            }
            max
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                walk_expr_nesting(arg, current_depth)
            } else {
                current_depth
            }
        }
        _ => current_depth,
    }
}

fn walk_expr_nesting(expr: &Expression<'_>, current_depth: u32) -> u32 {
    match expr {
        Expression::ConditionalExpression(cond) => {
            let depth = current_depth + 1;
            let d1 = walk_expr_nesting(&cond.consequent, depth);
            let d2 = walk_expr_nesting(&cond.alternate, depth);
            depth.max(d1).max(d2)
        }
        // Don't recurse into function expressions - they are separate
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => current_depth,
        _ => current_depth,
    }
}

/// Unused export for top-level analysis (uses statement slice)
#[allow(dead_code)]
pub fn analyze_nesting_depth_stmts(stmts: &[Statement<'_>]) -> u32 {
    analyze_max_nesting_depth(stmts)
}
