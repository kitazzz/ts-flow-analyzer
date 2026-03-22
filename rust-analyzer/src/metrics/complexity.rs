use oxc_ast::ast::*;

pub struct BasicComplexity {
    pub if_count: u32,
    pub else_if_count: u32,
    pub switch_count: u32,
    pub ternary_count: u32,
    pub return_count: u32,
    pub logical_operator_count: u32,
    pub negation_count: u32,
    pub atomic_condition_count: u32,
    pub max_condition_depth: u32,
}

pub fn analyze_basic_complexity(stmts: &[Statement<'_>]) -> BasicComplexity {
    let mut result = BasicComplexity {
        if_count: 0,
        else_if_count: 0,
        switch_count: 0,
        ternary_count: 0,
        return_count: 0,
        logical_operator_count: 0,
        negation_count: 0,
        atomic_condition_count: 0,
        max_condition_depth: 0,
    };
    for stmt in stmts {
        walk_stmt(stmt, false, &mut result);
    }
    result
}

/// Count atomic conditions and max logical nesting depth in a condition expression.
/// An atomic condition is a leaf expression that isn't a logical operator.
/// Condition depth is the max nesting of && / || operators.
fn analyze_condition(expr: &Expression<'_>, result: &mut BasicComplexity) {
    let (atomics, depth) = count_condition_parts(expr, 0);
    result.atomic_condition_count += atomics;
    if depth > result.max_condition_depth {
        result.max_condition_depth = depth;
    }
}

fn count_condition_parts(expr: &Expression<'_>, current_depth: u32) -> (u32, u32) {
    match expr {
        Expression::LogicalExpression(log) => {
            let new_depth = current_depth + 1;
            let (left_atoms, left_depth) = count_condition_parts(&log.left, new_depth);
            let (right_atoms, right_depth) = count_condition_parts(&log.right, new_depth);
            (left_atoms + right_atoms, left_depth.max(right_depth))
        }
        Expression::ParenthesizedExpression(inner) => {
            count_condition_parts(&inner.expression, current_depth)
        }
        Expression::UnaryExpression(u) if matches!(u.operator, UnaryOperator::LogicalNot) => {
            count_condition_parts(&u.argument, current_depth)
        }
        // Leaf: this is an atomic condition
        _ => (1, current_depth),
    }
}

fn walk_stmt(stmt: &Statement<'_>, skip_nested_fn: bool, result: &mut BasicComplexity) {
    match stmt {
        Statement::IfStatement(if_stmt) => {
            result.if_count += 1;
            analyze_condition(&if_stmt.test, result);
            walk_expr(&if_stmt.test, result);
            walk_stmt_branch(&if_stmt.consequent, result);
            if let Some(alt) = &if_stmt.alternate {
                // Check if else-if
                if matches!(alt, Statement::IfStatement(_)) {
                    result.else_if_count += 1;
                }
                walk_stmt(alt, false, result);
            }
        }
        Statement::SwitchStatement(sw) => {
            result.switch_count += 1;
            walk_expr(&sw.discriminant, result);
            for case in &sw.cases {
                if let Some(test) = &case.test {
                    walk_expr(test, result);
                }
                for s in &case.consequent {
                    walk_stmt(s, false, result);
                }
            }
        }
        Statement::ReturnStatement(ret) => {
            result.return_count += 1;
            if let Some(arg) = &ret.argument {
                walk_expr(arg, result);
            }
        }
        Statement::ThrowStatement(thr) => {
            walk_expr(&thr.argument, result);
        }
        Statement::BlockStatement(block) => {
            for s in &block.body {
                walk_stmt(s, false, result);
            }
        }
        Statement::WhileStatement(w) => {
            analyze_condition(&w.test, result);
            walk_expr(&w.test, result);
            walk_stmt(&w.body, false, result);
        }
        Statement::DoWhileStatement(dw) => {
            analyze_condition(&dw.test, result);
            walk_expr(&dw.test, result);
            walk_stmt(&dw.body, false, result);
        }
        Statement::ForStatement(f) => {
            if let Some(test) = &f.test {
                analyze_condition(test, result);
                walk_expr(test, result);
            }
            walk_stmt(&f.body, false, result);
        }
        Statement::ForInStatement(f) => {
            walk_stmt(&f.body, false, result);
        }
        Statement::ForOfStatement(f) => {
            walk_stmt(&f.body, false, result);
        }
        Statement::TryStatement(t) => {
            for s in &t.block.body {
                walk_stmt(s, false, result);
            }
            if let Some(handler) = &t.handler {
                for s in &handler.body.body {
                    walk_stmt(s, false, result);
                }
            }
            if let Some(fin) = &t.finalizer {
                for s in &fin.body {
                    walk_stmt(s, false, result);
                }
            }
        }
        Statement::ExpressionStatement(expr_stmt) => {
            walk_expr(&expr_stmt.expression, result);
        }
        Statement::VariableDeclaration(var_decl) => {
            for decl in &var_decl.declarations {
                if let Some(init) = &decl.init {
                    walk_expr_skip_fn(init, result);
                }
            }
        }
        // Skip nested function/arrow declarations
        Statement::FunctionDeclaration(_) => {
            if !skip_nested_fn {
                // don't recurse into nested function bodies
            }
        }
        _ => {}
    }
}

fn walk_stmt_branch(stmt: &Statement<'_>, result: &mut BasicComplexity) {
    walk_stmt(stmt, false, result);
}

fn walk_expr(expr: &Expression<'_>, result: &mut BasicComplexity) {
    match expr {
        Expression::ConditionalExpression(cond) => {
            result.ternary_count += 1;
            analyze_condition(&cond.test, result);
            walk_expr(&cond.test, result);
            walk_expr(&cond.consequent, result);
            walk_expr(&cond.alternate, result);
        }
        Expression::LogicalExpression(log) => {
            result.logical_operator_count += 1;
            walk_expr(&log.left, result);
            walk_expr(&log.right, result);
        }
        Expression::UnaryExpression(unary) => {
            if matches!(unary.operator, UnaryOperator::LogicalNot) {
                result.negation_count += 1;
            }
            walk_expr(&unary.argument, result);
        }
        Expression::BinaryExpression(bin) => {
            walk_expr(&bin.left, result);
            walk_expr(&bin.right, result);
        }
        Expression::CallExpression(call) => {
            walk_expr_skip_fn(&call.callee, result);
            for arg in &call.arguments {
                match arg {
                    Argument::SpreadElement(spread) => walk_expr_skip_fn(&spread.argument, result),
                    expr => walk_expr_skip_fn(expr.to_expression(), result),
                }
            }
        }
        Expression::AwaitExpression(aw) => {
            walk_expr(&aw.argument, result);
        }
        Expression::AssignmentExpression(assign) => {
            walk_expr(&assign.right, result);
        }
        Expression::TSNonNullExpression(inner) => {
            walk_expr(&inner.expression, result);
        }
        Expression::TSAsExpression(inner) => {
            walk_expr(&inner.expression, result);
        }
        Expression::ParenthesizedExpression(inner) => {
            walk_expr(&inner.expression, result);
        }
        // Don't recurse into nested function expressions
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {}
        _ => {}
    }
}

/// Walk expression but skip function bodies for complexity counting
fn walk_expr_skip_fn(expr: &Expression<'_>, result: &mut BasicComplexity) {
    walk_expr(expr, result);
}
