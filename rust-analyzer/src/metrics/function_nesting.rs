use oxc_ast::ast::*;

pub struct FunctionNestingResult {
    pub local_function_count: u32,
    pub function_nesting_depth: u32,
    pub callback_nesting_depth: u32,
}

pub fn analyze_function_nesting(stmts: &[Statement<'_>]) -> FunctionNestingResult {
    let local_function_count = count_local_functions_stmts(stmts);
    let function_nesting_depth = calc_function_nesting_depth_stmts(stmts);
    let callback_nesting_depth = calc_callback_nesting_depth_stmts(stmts, 0);

    FunctionNestingResult {
        local_function_count,
        function_nesting_depth,
        callback_nesting_depth,
    }
}

fn count_local_functions_stmts(stmts: &[Statement<'_>]) -> u32 {
    let mut count = 0;
    for stmt in stmts {
        count += count_local_functions_stmt(stmt);
    }
    count
}

fn count_local_functions_stmt(stmt: &Statement<'_>) -> u32 {
    match stmt {
        Statement::FunctionDeclaration(_) => 1,
        Statement::IfStatement(s) => {
            count_local_functions_stmt(&s.consequent)
                + s.alternate.as_ref().map_or(0, |a| count_local_functions_stmt(a))
        }
        Statement::BlockStatement(b) => count_local_functions_stmts(&b.body),
        Statement::WhileStatement(w) => count_local_functions_stmt(&w.body),
        Statement::DoWhileStatement(dw) => count_local_functions_stmt(&dw.body),
        Statement::ForStatement(f) => count_local_functions_stmt(&f.body),
        Statement::ForInStatement(f) => count_local_functions_stmt(&f.body),
        Statement::ForOfStatement(f) => count_local_functions_stmt(&f.body),
        Statement::SwitchStatement(sw) => {
            sw.cases.iter().map(|c| count_local_functions_stmts(&c.consequent)).sum()
        }
        Statement::TryStatement(t) => {
            count_local_functions_stmts(&t.block.body)
                + t.handler.as_ref().map_or(0, |h| count_local_functions_stmts(&h.body.body))
                + t.finalizer.as_ref().map_or(0, |f| count_local_functions_stmts(&f.body))
        }
        Statement::ReturnStatement(ret) => {
            ret.argument.as_ref().map_or(0, |a| count_local_functions_expr(a))
        }
        Statement::ExpressionStatement(expr_stmt) => {
            count_local_functions_expr(&expr_stmt.expression)
        }
        Statement::VariableDeclaration(var_decl) => {
            var_decl.declarations.iter().map(|d| {
                d.init.as_ref().map_or(0, |i| count_local_functions_expr(i))
            }).sum()
        }
        _ => 0,
    }
}

fn count_local_functions_expr(expr: &Expression<'_>) -> u32 {
    match expr {
        Expression::FunctionExpression(_) => 1,
        Expression::ArrowFunctionExpression(_) => 1,
        Expression::CallExpression(call) => {
            let mut count = count_local_functions_expr(&call.callee);
            for arg in &call.arguments {
                match arg {
                    Argument::SpreadElement(spread) => count += count_local_functions_expr(&spread.argument),
                    expr => count += count_local_functions_expr(expr.to_expression()),
                }
            }
            count
        }
        Expression::LogicalExpression(log) => {
            count_local_functions_expr(&log.left) + count_local_functions_expr(&log.right)
        }
        Expression::ConditionalExpression(cond) => {
            count_local_functions_expr(&cond.test)
                + count_local_functions_expr(&cond.consequent)
                + count_local_functions_expr(&cond.alternate)
        }
        Expression::AwaitExpression(aw) => count_local_functions_expr(&aw.argument),
        Expression::TSNonNullExpression(inner) => count_local_functions_expr(&inner.expression),
        Expression::TSAsExpression(inner) => count_local_functions_expr(&inner.expression),
        Expression::ParenthesizedExpression(inner) => count_local_functions_expr(&inner.expression),
        _ => 0,
    }
}

fn calc_function_nesting_depth_stmts(stmts: &[Statement<'_>]) -> u32 {
    let mut max = 0;
    for stmt in stmts {
        let d = walk_fn_depth_stmt(stmt, 0, true);
        if d > max { max = d; }
    }
    max
}

fn walk_fn_depth_stmt(stmt: &Statement<'_>, depth: u32, is_root: bool) -> u32 {
    match stmt {
        Statement::FunctionDeclaration(f) => {
            let new_depth = if is_root { depth } else { depth + 1 };
            if let Some(body) = &f.body {
                walk_fn_depth_stmts(&body.statements, new_depth, false)
            } else {
                new_depth
            }
        }
        Statement::BlockStatement(b) => walk_fn_depth_stmts(&b.body, depth, false),
        Statement::IfStatement(s) => {
            let d1 = walk_fn_depth_stmt(&s.consequent, depth, false);
            let d2 = s.alternate.as_ref().map_or(depth, |a| walk_fn_depth_stmt(a, depth, false));
            d1.max(d2)
        }
        Statement::WhileStatement(w) => walk_fn_depth_stmt(&w.body, depth, false),
        Statement::DoWhileStatement(dw) => walk_fn_depth_stmt(&dw.body, depth, false),
        Statement::ForStatement(f) => walk_fn_depth_stmt(&f.body, depth, false),
        Statement::ForInStatement(f) => walk_fn_depth_stmt(&f.body, depth, false),
        Statement::ForOfStatement(f) => walk_fn_depth_stmt(&f.body, depth, false),
        Statement::SwitchStatement(sw) => {
            sw.cases.iter().map(|c| walk_fn_depth_stmts(&c.consequent, depth, false)).max().unwrap_or(depth)
        }
        Statement::TryStatement(t) => {
            let d1 = walk_fn_depth_stmts(&t.block.body, depth, false);
            let d2 = t.handler.as_ref().map_or(depth, |h| walk_fn_depth_stmts(&h.body.body, depth, false));
            let d3 = t.finalizer.as_ref().map_or(depth, |f| walk_fn_depth_stmts(&f.body, depth, false));
            d1.max(d2).max(d3)
        }
        Statement::ReturnStatement(ret) => {
            ret.argument.as_ref().map_or(depth, |a| walk_fn_depth_expr(a, depth, false))
        }
        Statement::ExpressionStatement(expr_stmt) => {
            walk_fn_depth_expr(&expr_stmt.expression, depth, false)
        }
        Statement::VariableDeclaration(var_decl) => {
            var_decl.declarations.iter().map(|d| {
                d.init.as_ref().map_or(depth, |i| walk_fn_depth_expr(i, depth, false))
            }).max().unwrap_or(depth)
        }
        _ => depth,
    }
}

fn walk_fn_depth_stmts(stmts: &[Statement<'_>], depth: u32, is_root: bool) -> u32 {
    let mut max = depth;
    for stmt in stmts {
        let d = walk_fn_depth_stmt(stmt, depth, is_root);
        if d > max { max = d; }
    }
    max
}

fn walk_fn_depth_expr(expr: &Expression<'_>, depth: u32, _is_root: bool) -> u32 {
    match expr {
        Expression::FunctionExpression(f) => {
            let new_depth = depth + 1;
            if let Some(body) = &f.body {
                walk_fn_depth_stmts(&body.statements, new_depth, false)
            } else {
                new_depth
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            let new_depth = depth + 1;
            walk_fn_depth_stmts(&arrow.body.statements, new_depth, false)
        }
        Expression::CallExpression(call) => {
            let mut max = walk_fn_depth_expr(&call.callee, depth, false);
            for arg in &call.arguments {
                let d = match arg {
                    Argument::SpreadElement(spread) => walk_fn_depth_expr(&spread.argument, depth, false),
                    expr => walk_fn_depth_expr(expr.to_expression(), depth, false),
                };
                if d > max { max = d; }
            }
            max
        }
        Expression::LogicalExpression(log) => {
            walk_fn_depth_expr(&log.left, depth, false)
                .max(walk_fn_depth_expr(&log.right, depth, false))
        }
        Expression::ConditionalExpression(cond) => {
            walk_fn_depth_expr(&cond.test, depth, false)
                .max(walk_fn_depth_expr(&cond.consequent, depth, false))
                .max(walk_fn_depth_expr(&cond.alternate, depth, false))
        }
        Expression::AwaitExpression(aw) => walk_fn_depth_expr(&aw.argument, depth, false),
        Expression::TSNonNullExpression(inner) => walk_fn_depth_expr(&inner.expression, depth, false),
        Expression::TSAsExpression(inner) => walk_fn_depth_expr(&inner.expression, depth, false),
        Expression::ParenthesizedExpression(inner) => walk_fn_depth_expr(&inner.expression, depth, false),
        _ => depth,
    }
}

fn calc_callback_nesting_depth_stmts(stmts: &[Statement<'_>], depth: u32) -> u32 {
    let mut max = depth;
    for stmt in stmts {
        let d = calc_callback_nesting_depth_stmt(stmt, depth);
        if d > max { max = d; }
    }
    max
}

fn calc_callback_nesting_depth_stmt(stmt: &Statement<'_>, depth: u32) -> u32 {
    match stmt {
        Statement::BlockStatement(b) => calc_callback_nesting_depth_stmts(&b.body, depth),
        Statement::IfStatement(s) => {
            let d1 = calc_callback_nesting_depth_stmt(&s.consequent, depth);
            let d2 = s.alternate.as_ref().map_or(depth, |a| calc_callback_nesting_depth_stmt(a, depth));
            d1.max(d2)
        }
        Statement::WhileStatement(w) => calc_callback_nesting_depth_stmt(&w.body, depth),
        Statement::DoWhileStatement(dw) => calc_callback_nesting_depth_stmt(&dw.body, depth),
        Statement::ForStatement(f) => calc_callback_nesting_depth_stmt(&f.body, depth),
        Statement::ForInStatement(f) => calc_callback_nesting_depth_stmt(&f.body, depth),
        Statement::ForOfStatement(f) => calc_callback_nesting_depth_stmt(&f.body, depth),
        Statement::SwitchStatement(sw) => {
            sw.cases.iter().map(|c| calc_callback_nesting_depth_stmts(&c.consequent, depth)).max().unwrap_or(depth)
        }
        Statement::TryStatement(t) => {
            let d1 = calc_callback_nesting_depth_stmts(&t.block.body, depth);
            let d2 = t.handler.as_ref().map_or(depth, |h| calc_callback_nesting_depth_stmts(&h.body.body, depth));
            let d3 = t.finalizer.as_ref().map_or(depth, |f| calc_callback_nesting_depth_stmts(&f.body, depth));
            d1.max(d2).max(d3)
        }
        Statement::ReturnStatement(ret) => {
            ret.argument.as_ref().map_or(depth, |a| calc_callback_nesting_expr(a, depth))
        }
        Statement::ExpressionStatement(expr_stmt) => {
            calc_callback_nesting_expr(&expr_stmt.expression, depth)
        }
        Statement::VariableDeclaration(var_decl) => {
            var_decl.declarations.iter().map(|d| {
                d.init.as_ref().map_or(depth, |i| calc_callback_nesting_expr(i, depth))
            }).max().unwrap_or(depth)
        }
        Statement::FunctionDeclaration(_) => depth,
        _ => depth,
    }
}

fn calc_callback_nesting_expr(expr: &Expression<'_>, depth: u32) -> u32 {
    match expr {
        Expression::CallExpression(call) => {
            let mut max = calc_callback_nesting_expr(&call.callee, depth);
            for arg in &call.arguments {
                let arg_expr = match arg {
                    Argument::SpreadElement(spread) => &spread.argument,
                    e => e.to_expression(),
                };
                let d = match arg_expr {
                    Expression::FunctionExpression(f) => {
                        // callback argument - increase depth
                        let new_depth = depth + 1;
                        if let Some(body) = &f.body {
                            calc_callback_nesting_depth_stmts(&body.statements, new_depth)
                        } else {
                            new_depth
                        }
                    }
                    Expression::ArrowFunctionExpression(arrow) => {
                        let new_depth = depth + 1;
                        calc_callback_nesting_depth_stmts(&arrow.body.statements, new_depth)
                    }
                    other => calc_callback_nesting_expr(other, depth),
                };
                if d > max { max = d; }
            }
            max
        }
        Expression::LogicalExpression(log) => {
            calc_callback_nesting_expr(&log.left, depth)
                .max(calc_callback_nesting_expr(&log.right, depth))
        }
        Expression::ConditionalExpression(cond) => {
            calc_callback_nesting_expr(&cond.test, depth)
                .max(calc_callback_nesting_expr(&cond.consequent, depth))
                .max(calc_callback_nesting_expr(&cond.alternate, depth))
        }
        Expression::AwaitExpression(aw) => calc_callback_nesting_expr(&aw.argument, depth),
        Expression::TSNonNullExpression(inner) => calc_callback_nesting_expr(&inner.expression, depth),
        Expression::TSAsExpression(inner) => calc_callback_nesting_expr(&inner.expression, depth),
        Expression::ParenthesizedExpression(inner) => calc_callback_nesting_expr(&inner.expression, depth),
        // Don't recurse into standalone function expressions (only callbacks increase depth)
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => depth,
        _ => depth,
    }
}
