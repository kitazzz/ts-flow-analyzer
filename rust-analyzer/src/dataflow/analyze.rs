use std::collections::{HashMap, HashSet};

use oxc_ast::ast::*;

use super::model::*;

type BindingId = u32;
/// binding_id → Vec<(def_id, may_reach)>
type ReachingSet = HashMap<BindingId, Vec<(DefId, bool)>>;

// ---------------------------------------------------------------------------
// Scope stack
// ---------------------------------------------------------------------------

struct Binding {
    name: String,
}

struct ScopeStack {
    function_bindings: HashMap<String, BindingId>,
    block_scopes: Vec<Vec<Binding>>,
    name_to_binding: HashMap<String, Vec<BindingId>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            function_bindings: HashMap::new(),
            block_scopes: Vec::new(),
            name_to_binding: HashMap::new(),
        }
    }

    fn push_block(&mut self) {
        self.block_scopes.push(Vec::new());
    }

    fn pop_block(&mut self) {
        if let Some(bindings) = self.block_scopes.pop() {
            for b in bindings {
                if let Some(stack) = self.name_to_binding.get_mut(&b.name) {
                    stack.pop();
                    if stack.is_empty() {
                        self.name_to_binding.remove(&b.name);
                    }
                }
            }
        }
    }

    fn declare_lexical(&mut self, name: &str, next_id: &mut BindingId) -> BindingId {
        let id = *next_id;
        *next_id += 1;
        if let Some(scope) = self.block_scopes.last_mut() {
            scope.push(Binding {
                name: name.to_string(),
            });
        }
        self.name_to_binding
            .entry(name.to_string())
            .or_default()
            .push(id);
        id
    }

    fn declare_var(&mut self, name: &str, next_id: &mut BindingId) -> BindingId {
        if let Some(&existing) = self.function_bindings.get(name) {
            return existing;
        }
        let id = *next_id;
        *next_id += 1;
        self.function_bindings.insert(name.to_string(), id);
        id
    }

    fn resolve(&self, name: &str) -> Option<BindingId> {
        if let Some(stack) = self.name_to_binding.get(name) {
            if let Some(&id) = stack.last() {
                return Some(id);
            }
        }
        self.function_bindings.get(name).copied()
    }
}

// ---------------------------------------------------------------------------
// Reaching set merge
// ---------------------------------------------------------------------------

fn merge_reaching(branches: &[ReachingSet]) -> ReachingSet {
    if branches.is_empty() {
        return HashMap::new();
    }
    if branches.len() == 1 {
        return branches[0].clone();
    }
    let mut result: ReachingSet = HashMap::new();
    // Collect all binding ids
    let mut all_bindings: Vec<BindingId> = Vec::new();
    for br in branches {
        for &bid in br.keys() {
            if !all_bindings.contains(&bid) {
                all_bindings.push(bid);
            }
        }
    }
    for bid in all_bindings {
        let mut merged: Vec<(DefId, bool)> = Vec::new();
        for br in branches {
            if let Some(defs) = br.get(&bid) {
                for &(def_id, mr) in defs {
                    if let Some(existing) = merged.iter_mut().find(|(d, _)| *d == def_id) {
                        // def exists in multiple branches — keep the more conservative may_reach
                        // If already may_reach or was may_reach in this branch, stays may_reach
                        existing.1 = existing.1 || mr;
                    } else {
                        // def only in this branch so far — mark as may_reach
                        merged.push((def_id, true));
                    }
                }
            }
        }
        // For defs that exist in ALL branches, check if they were all must-reach
        for entry in &mut merged {
            let present_in_all = branches.iter().all(|br| {
                br.get(&bid)
                    .map_or(false, |defs| defs.iter().any(|(d, _)| *d == entry.0))
            });
            if present_in_all {
                // Collect the original may_reach values
                let any_may = branches.iter().any(|br| {
                    br.get(&bid)
                        .and_then(|defs| defs.iter().find(|(d, _)| *d == entry.0))
                        .map_or(false, |(_, mr)| *mr)
                });
                entry.1 = any_may;
            }
            // If not present in all, it stays true (set above)
        }
        if !merged.is_empty() {
            result.insert(bid, merged);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Flow walker
// ---------------------------------------------------------------------------

struct FlowWalker<'a> {
    source: &'a str,
    next_def_id: DefId,
    next_use_id: UseId,
    next_binding_id: BindingId,
    scope: ScopeStack,
    reaching: ReachingSet,
    this_field_bindings: HashMap<String, BindingId>,
    def_intern: HashMap<(String, u32), DefId>,
    def_registered: HashMap<DefId, bool>,
    /// Span starts of function declarations hoisted by hoist_function_decls (body-level only).
    /// Used to distinguish body-level hoisted fns from block-level fns with the same name.
    hoisted_fn_spans: HashSet<u32>,
    dry_run: bool,
    defs: Vec<Def>,
    uses: Vec<Use>,
    edges: Vec<DefUseEdge>,
}

impl<'a> FlowWalker<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            next_def_id: 0,
            next_use_id: 0,
            next_binding_id: 0,
            scope: ScopeStack::new(),
            reaching: HashMap::new(),
            this_field_bindings: HashMap::new(),
            def_intern: HashMap::new(),
            def_registered: HashMap::new(),
            hoisted_fn_spans: HashSet::new(),
            dry_run: false,
            defs: Vec::new(),
            uses: Vec::new(),
            edges: Vec::new(),
        }
    }

    fn span_line(&self, start: u32) -> u32 {
        self.source[..start as usize]
            .bytes()
            .filter(|b| *b == b'\n')
            .count() as u32
            + 1
    }

    fn get_or_create_this_field_binding(&mut self, name: &str) -> BindingId {
        if let Some(&id) = self.this_field_bindings.get(name) {
            return id;
        }
        let id = self.next_binding_id;
        self.next_binding_id += 1;
        self.this_field_bindings.insert(name.to_string(), id);
        id
    }

    // ----- def / use emission -----

    fn emit_def(&mut self, name: String, kind: DefKind, span_start: u32, span_end: u32) -> DefId {
        let key = (name.clone(), span_start);
        let line = self.span_line(span_start);

        let id = if let Some(&existing_id) = self.def_intern.get(&key) {
            existing_id
        } else {
            let id = self.next_def_id;
            self.next_def_id += 1;
            self.def_intern.insert(key, id);
            id
        };

        if !self.def_registered.contains_key(&id) {
            self.def_registered.insert(id, true);
            self.defs.push(Def {
                id,
                name: name.clone(),
                kind,
                line,
                span_start,
                span_end,
            });
        }

        // Resolve binding id
        let binding_id = if name.starts_with("this.") {
            self.get_or_create_this_field_binding(&name)
        } else {
            self.scope.resolve(&name).unwrap_or_else(|| {
                // Shouldn't happen if scope is correct, but fallback
                self.scope.declare_var(&name, &mut self.next_binding_id)
            })
        };

        // Kill + gen in reaching set
        self.reaching.insert(binding_id, vec![(id, false)]);
        id
    }

    fn record_def_no_reach(
        &mut self,
        name: String,
        kind: DefKind,
        span_start: u32,
        span_end: u32,
    ) -> DefId {
        let key = (name.clone(), span_start);
        let line = self.span_line(span_start);

        let id = if let Some(&existing_id) = self.def_intern.get(&key) {
            existing_id
        } else {
            let id = self.next_def_id;
            self.next_def_id += 1;
            self.def_intern.insert(key, id);
            id
        };

        if !self.def_registered.contains_key(&id) {
            self.def_registered.insert(id, true);
            self.defs.push(Def {
                id,
                name,
                kind,
                line,
                span_start,
                span_end,
            });
        }

        id
    }

    fn emit_use(&mut self, name: String, kind: UseKind, span_start: u32, span_end: u32) {
        if self.dry_run {
            return;
        }

        let use_id = self.next_use_id;
        self.next_use_id += 1;
        let line = self.span_line(span_start);
        self.uses.push(Use {
            id: use_id,
            name: name.clone(),
            kind: kind.clone(),
            line,
            span_start,
            span_end,
        });

        let binding_id = if name.starts_with("this.") {
            Some(self.get_or_create_this_field_binding(&name))
        } else {
            self.scope.resolve(&name)
        };

        let Some(bid) = binding_id else { return };
        if let Some(reaching_defs) = self.reaching.get(&bid) {
            for &(def_id, may_reach) in reaching_defs {
                let def_line = self
                    .defs
                    .iter()
                    .find(|d| d.id == def_id)
                    .map(|d| d.line)
                    .unwrap_or(0);
                self.edges.push(DefUseEdge {
                    def_id,
                    use_id,
                    def_name: name.clone(),
                    def_line,
                    use_line: line,
                    use_kind: kind.clone(),
                    may_reach,
                });
            }
        }
    }

    // ----- var hoisting -----

    fn hoist_vars(&mut self, stmts: &[Statement<'_>]) {
        for stmt in stmts {
            self.hoist_vars_from_stmt(stmt);
        }
    }

    fn hoist_vars_from_stmt(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                if decl.kind == VariableDeclarationKind::Var {
                    for declarator in &decl.declarations {
                        let names = collect_binding_names(&declarator.id);
                        for (name, _span_start, _span_end) in names {
                            self.scope.declare_var(&name, &mut self.next_binding_id);
                        }
                    }
                }
            }
            Statement::BlockStatement(b) => self.hoist_vars(&b.body),
            Statement::IfStatement(s) => {
                self.hoist_vars_from_stmt(&s.consequent);
                if let Some(alt) = &s.alternate {
                    self.hoist_vars_from_stmt(alt);
                }
            }
            Statement::WhileStatement(w) => self.hoist_vars_from_stmt(&w.body),
            Statement::DoWhileStatement(dw) => self.hoist_vars_from_stmt(&dw.body),
            Statement::ForStatement(f) => {
                if let Some(ForStatementInit::VariableDeclaration(decl)) = &f.init {
                    if decl.kind == VariableDeclarationKind::Var {
                        for declarator in &decl.declarations {
                            let names = collect_binding_names(&declarator.id);
                            for (name, _s, _e) in names {
                                self.scope.declare_var(&name, &mut self.next_binding_id);
                            }
                        }
                    }
                }
                self.hoist_vars_from_stmt(&f.body);
            }
            Statement::ForInStatement(f) => {
                if let ForStatementLeft::VariableDeclaration(decl) = &f.left {
                    if decl.kind == VariableDeclarationKind::Var {
                        for declarator in &decl.declarations {
                            let names = collect_binding_names(&declarator.id);
                            for (name, _s, _e) in names {
                                self.scope.declare_var(&name, &mut self.next_binding_id);
                            }
                        }
                    }
                }
                self.hoist_vars_from_stmt(&f.body);
            }
            Statement::ForOfStatement(f) => {
                if let ForStatementLeft::VariableDeclaration(decl) = &f.left {
                    if decl.kind == VariableDeclarationKind::Var {
                        for declarator in &decl.declarations {
                            let names = collect_binding_names(&declarator.id);
                            for (name, _s, _e) in names {
                                self.scope.declare_var(&name, &mut self.next_binding_id);
                            }
                        }
                    }
                }
                self.hoist_vars_from_stmt(&f.body);
            }
            Statement::SwitchStatement(sw) => {
                for case in &sw.cases {
                    for s in &case.consequent {
                        self.hoist_vars_from_stmt(s);
                    }
                }
            }
            Statement::TryStatement(t) => {
                self.hoist_vars(&t.block.body);
                if let Some(handler) = &t.handler {
                    self.hoist_vars(&handler.body.body);
                }
                if let Some(fin) = &t.finalizer {
                    self.hoist_vars(&fin.body);
                }
            }
            Statement::LabeledStatement(l) => self.hoist_vars_from_stmt(&l.body),
            // FunctionDeclaration / ArrowFunctionExpression — boundary, do not recurse
            Statement::FunctionDeclaration(_) => {}
            _ => {}
        }
    }

    fn hoist_function_decls(&mut self, stmts: &[Statement<'_>]) {
        for stmt in stmts {
            if let Statement::FunctionDeclaration(f) = stmt {
                if let Some(ref id) = f.id {
                    let name = id.name.to_string();
                    self.hoisted_fn_spans.insert(f.span.start);
                    self.scope.declare_var(&name, &mut self.next_binding_id);
                    self.emit_def(
                        name,
                        DefKind::Declaration,
                        f.span.start,
                        f.span.end,
                    );
                }
            }
        }
    }

    // ----- parameters -----

    fn seed_params(&mut self, params: &FormalParameters<'_>) {
        for param in &params.items {
            // Walk default initializer first (can reference previously declared params)
            if let Some(ref init) = param.initializer {
                self.collect_uses_from_expr(init);
            }
            let names = collect_binding_names(&param.pattern);
            for (name, span_start, span_end) in names {
                self.scope.declare_var(&name, &mut self.next_binding_id);
                self.emit_def(name, DefKind::Parameter, span_start, span_end);
            }
        }
        // Handle rest parameter
        if let Some(ref rest) = params.rest {
            let names = collect_binding_names(&rest.rest.argument);
            for (name, span_start, span_end) in names {
                self.scope.declare_var(&name, &mut self.next_binding_id);
                self.emit_def(name, DefKind::Parameter, span_start, span_end);
            }
        }
    }

    // ----- statement walking -----

    fn walk_stmts(&mut self, stmts: &[Statement<'_>]) {
        for stmt in stmts {
            self.walk_stmt(stmt);
        }
    }

    fn walk_stmt(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                self.walk_var_declaration(decl);
            }
            Statement::ExpressionStatement(expr_stmt) => {
                self.collect_uses_from_expr(&expr_stmt.expression);
            }
            Statement::ReturnStatement(ret) => {
                if let Some(ref arg) = ret.argument {
                    self.collect_uses_from_expr(arg);
                }
            }
            Statement::ThrowStatement(thr) => {
                self.collect_uses_from_expr(&thr.argument);
            }
            Statement::BlockStatement(b) => {
                self.scope.push_block();
                self.walk_stmts(&b.body);
                self.scope.pop_block();
            }
            Statement::IfStatement(s) => {
                self.collect_uses_from_expr(&s.test);
                let reaching_before = self.reaching.clone();

                // Then branch
                self.walk_stmt(&s.consequent);
                let reaching_then = self.reaching.clone();

                // Else branch
                self.reaching = reaching_before;
                if let Some(ref alt) = s.alternate {
                    self.walk_stmt(alt);
                }
                let reaching_else = self.reaching.clone();

                self.reaching = merge_reaching(&[reaching_then, reaching_else]);
            }
            Statement::SwitchStatement(sw) => {
                self.collect_uses_from_expr(&sw.discriminant);
                self.scope.push_block();
                // Fallthrough: sequential walk of all cases
                for case in &sw.cases {
                    if let Some(ref test) = case.test {
                        self.collect_uses_from_expr(test);
                    }
                    for s in &case.consequent {
                        self.walk_stmt(s);
                    }
                }
                self.scope.pop_block();
            }
            Statement::WhileStatement(w) => {
                // dry-run: test + body
                let reaching_before = self.reaching.clone();
                self.dry_run = true;
                self.collect_uses_from_expr(&w.test);
                self.walk_stmt(&w.body);
                self.dry_run = false;
                self.reaching = merge_reaching(&[reaching_before, self.reaching.clone()]);

                // Real walk
                self.collect_uses_from_expr(&w.test);
                self.walk_stmt(&w.body);
            }
            Statement::DoWhileStatement(dw) => {
                // dry-run: body + test
                let reaching_before = self.reaching.clone();
                self.dry_run = true;
                self.walk_stmt(&dw.body);
                self.collect_uses_from_expr(&dw.test);
                self.dry_run = false;
                self.reaching = merge_reaching(&[reaching_before, self.reaching.clone()]);

                // Real walk
                self.walk_stmt(&dw.body);
                self.collect_uses_from_expr(&dw.test);
            }
            Statement::ForStatement(f) => {
                self.scope.push_block();

                // init
                if let Some(ref init) = f.init {
                    match init {
                        ForStatementInit::VariableDeclaration(var_decl) => {
                            self.walk_var_declaration(var_decl);
                        }
                        _ => {
                            self.collect_uses_from_for_init(init);
                        }
                    }
                }

                // dry-run: test + body + update
                let reaching_before = self.reaching.clone();
                self.dry_run = true;
                if let Some(ref test) = f.test {
                    self.collect_uses_from_expr(test);
                }
                self.walk_stmt(&f.body);
                if let Some(ref update) = f.update {
                    self.collect_uses_from_expr(update);
                }
                self.dry_run = false;
                self.reaching = merge_reaching(&[reaching_before, self.reaching.clone()]);

                // Real walk: test + body + update
                if let Some(ref test) = f.test {
                    self.collect_uses_from_expr(test);
                }
                self.walk_stmt(&f.body);
                if let Some(ref update) = f.update {
                    self.collect_uses_from_expr(update);
                }

                self.scope.pop_block();
            }
            Statement::ForInStatement(f) => {
                self.scope.push_block();

                // right expression (evaluated first)
                self.collect_uses_from_expr(&f.right);

                // Declare left binding
                self.declare_for_left(&f.left);

                // dry-run: body
                let reaching_before = self.reaching.clone();
                self.dry_run = true;
                self.walk_stmt(&f.body);
                self.dry_run = false;
                self.reaching = merge_reaching(&[reaching_before, self.reaching.clone()]);

                // Re-declare for left (reaching set for loop binding)
                self.redeclare_for_left(&f.left);

                // Real walk
                self.walk_stmt(&f.body);

                self.scope.pop_block();
            }
            Statement::ForOfStatement(f) => {
                self.scope.push_block();

                self.collect_uses_from_expr(&f.right);
                self.declare_for_left(&f.left);

                let reaching_before = self.reaching.clone();
                self.dry_run = true;
                self.walk_stmt(&f.body);
                self.dry_run = false;
                self.reaching = merge_reaching(&[reaching_before, self.reaching.clone()]);

                self.redeclare_for_left(&f.left);
                self.walk_stmt(&f.body);

                self.scope.pop_block();
            }
            Statement::TryStatement(t) => {
                // try block
                self.walk_stmts(&t.block.body);
                let reaching_after_try = self.reaching.clone();

                // catch handler
                if let Some(ref handler) = t.handler {
                    self.scope.push_block();
                    if let Some(ref param) = handler.param {
                        let names = collect_binding_names(&param.pattern);
                        for (name, span_start, span_end) in names {
                            self.scope
                                .declare_lexical(&name, &mut self.next_binding_id);
                            self.emit_def(name, DefKind::CatchBinding, span_start, span_end);
                        }
                    }
                    self.walk_stmts(&handler.body.body);
                    self.scope.pop_block();
                }
                let reaching_after_catch = self.reaching.clone();

                self.reaching =
                    merge_reaching(&[reaching_after_try, reaching_after_catch]);

                // finalizer
                if let Some(ref fin) = t.finalizer {
                    self.walk_stmts(&fin.body);
                }
            }
            Statement::FunctionDeclaration(f) => {
                // Body-level function decls were already handled by hoist_function_decls.
                // Block-level function decls: declare_lexical + emit_def.
                // Use hoisted_fn_spans (not function_bindings) to distinguish,
                // because params/vars also live in function_bindings.
                if let Some(ref id) = f.id {
                    let name = id.name.to_string();
                    if !self.hoisted_fn_spans.contains(&f.span.start) {
                        // Block-level: declare as lexical (shadows any outer binding)
                        self.scope
                            .declare_lexical(&name, &mut self.next_binding_id);
                        self.emit_def(
                            name,
                            DefKind::Declaration,
                            f.span.start,
                            f.span.end,
                        );
                    }
                    // Do NOT recurse into function body (boundary)
                }
            }
            Statement::LabeledStatement(l) => {
                self.walk_stmt(&l.body);
            }
            _ => {}
        }
    }

    fn walk_var_declaration(&mut self, decl: &VariableDeclaration<'_>) {
        let is_var = decl.kind == VariableDeclarationKind::Var;
        for declarator in &decl.declarations {
            // Walk initializer first
            if let Some(ref init) = declarator.init {
                self.collect_uses_from_expr(init);
            }

            let names = collect_binding_names(&declarator.id);
            let has_init = declarator.init.is_some();
            let kind = if names.len() > 1 {
                DefKind::Destructuring
            } else {
                DefKind::Declaration
            };

            for (name, span_start, span_end) in names {
                if is_var {
                    // Already declared in function scope by hoist_vars
                    // Just ensure it's accessible (declare_var returns existing)
                    self.scope.declare_var(&name, &mut self.next_binding_id);
                } else {
                    self.scope
                        .declare_lexical(&name, &mut self.next_binding_id);
                }

                if has_init {
                    self.emit_def(name, kind.clone(), span_start, span_end);
                } else {
                    self.record_def_no_reach(name, kind.clone(), span_start, span_end);
                }
            }
        }
    }

    fn declare_for_left(&mut self, left: &ForStatementLeft<'_>) {
        match left {
            ForStatementLeft::VariableDeclaration(decl) => {
                let is_var = decl.kind == VariableDeclarationKind::Var;
                for declarator in &decl.declarations {
                    let names = collect_binding_names(&declarator.id);
                    for (name, span_start, span_end) in names {
                        if is_var {
                            self.scope.declare_var(&name, &mut self.next_binding_id);
                        } else {
                            self.scope
                                .declare_lexical(&name, &mut self.next_binding_id);
                        }
                        self.emit_def(name, DefKind::ForBinding, span_start, span_end);
                    }
                }
            }
            _ => {
                // Non-declaration left: `for (x in obj)` or `for (x of xs)`
                // The left is an AssignmentTarget — emit defs for identifiers
                self.emit_defs_from_for_left_target(left);
            }
        }
    }

    fn redeclare_for_left(&mut self, left: &ForStatementLeft<'_>) {
        match left {
            ForStatementLeft::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    let names = collect_binding_names(&declarator.id);
                    for (name, span_start, span_end) in names {
                        self.emit_def(name, DefKind::ForBinding, span_start, span_end);
                    }
                }
            }
            _ => {
                self.emit_defs_from_for_left_target(left);
            }
        }
    }

    /// Emit defs for non-declaration for-in/for-of left (e.g. `for (x in obj)`)
    fn emit_defs_from_for_left_target(&mut self, left: &ForStatementLeft<'_>) {
        // ForStatementLeft inherits AssignmentTarget variants
        match left {
            ForStatementLeft::VariableDeclaration(_) => {}
            ForStatementLeft::AssignmentTargetIdentifier(id) => {
                let name = id.name.to_string();
                self.emit_def(name, DefKind::ForBinding, id.span.start, id.span.end);
            }
            ForStatementLeft::ObjectAssignmentTarget(obj) => {
                let names = collect_assignment_target_names_from_object(obj);
                for (name, span_start, span_end) in names {
                    self.emit_def(name, DefKind::ForBinding, span_start, span_end);
                }
            }
            ForStatementLeft::ArrayAssignmentTarget(arr) => {
                let names = collect_assignment_target_names_from_array(arr);
                for (name, span_start, span_end) in names {
                    self.emit_def(name, DefKind::ForBinding, span_start, span_end);
                }
            }
            _ => {}
        }
    }

    fn collect_uses_from_for_init(&mut self, init: &ForStatementInit<'_>) {
        // ForStatementInit inherits Expression variants
        match init {
            ForStatementInit::VariableDeclaration(_) => {} // handled separately
            _ => {
                // It's an expression variant - use GetSpan to extract span
                // We need to walk it as an expression. ForStatementInit inherits Expression.
                self.walk_for_init_expr(init);
            }
        }
    }

    fn walk_for_init_expr(&mut self, init: &ForStatementInit<'_>) {
        match init {
            ForStatementInit::VariableDeclaration(_) => {}
            _ => {
                self.collect_uses_from_expr(init.to_expression());
            }
        }
    }

    // ----- expression walking -----

    fn collect_uses_from_expr(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                if is_builtin_global(name) {
                    return;
                }
                self.emit_use(
                    name.to_string(),
                    UseKind::Read,
                    id.span.start,
                    id.span.end,
                );
            }
            Expression::StaticMemberExpression(m) if is_this(&m.object) => {
                let name = format!("this.{}", m.property.name);
                self.emit_use(name, UseKind::MemberRead, m.span.start, m.span.end);
            }
            Expression::AssignmentExpression(a) => {
                self.handle_assignment_expr(a);
            }
            Expression::UpdateExpression(u) => {
                self.handle_update_expr(u);
            }
            // Recurse into sub-expressions
            Expression::BinaryExpression(b) => {
                self.collect_uses_from_expr(&b.left);
                self.collect_uses_from_expr(&b.right);
            }
            Expression::LogicalExpression(l) => {
                self.collect_uses_from_expr(&l.left);
                self.collect_uses_from_expr(&l.right);
            }
            Expression::ConditionalExpression(c) => {
                self.collect_uses_from_expr(&c.test);
                self.collect_uses_from_expr(&c.consequent);
                self.collect_uses_from_expr(&c.alternate);
            }
            Expression::CallExpression(c) => {
                self.collect_uses_from_expr(&c.callee);
                for arg in &c.arguments {
                    self.collect_uses_from_argument(arg);
                }
            }
            Expression::NewExpression(n) => {
                self.collect_uses_from_expr(&n.callee);
                for arg in &n.arguments {
                    self.collect_uses_from_argument(arg);
                }
            }
            Expression::StaticMemberExpression(m) => {
                self.collect_uses_from_expr(&m.object);
            }
            Expression::ComputedMemberExpression(m) => {
                self.collect_uses_from_expr(&m.object);
                self.collect_uses_from_expr(&m.expression);
            }
            Expression::UnaryExpression(u) => {
                self.collect_uses_from_expr(&u.argument);
            }
            Expression::AwaitExpression(a) => {
                self.collect_uses_from_expr(&a.argument);
            }
            Expression::YieldExpression(y) => {
                if let Some(ref arg) = y.argument {
                    self.collect_uses_from_expr(arg);
                }
            }
            Expression::SequenceExpression(s) => {
                for expr in &s.expressions {
                    self.collect_uses_from_expr(expr);
                }
            }
            Expression::ParenthesizedExpression(p) => {
                self.collect_uses_from_expr(&p.expression);
            }
            Expression::ArrayExpression(a) => {
                for elem in &a.elements {
                    self.collect_uses_from_array_element(elem);
                }
            }
            Expression::ObjectExpression(o) => {
                for prop in &o.properties {
                    match prop {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            if p.computed {
                                self.collect_uses_from_property_key(&p.key);
                            }
                            self.collect_uses_from_expr(&p.value);
                        }
                        ObjectPropertyKind::SpreadProperty(s) => {
                            self.collect_uses_from_expr(&s.argument);
                        }
                    }
                }
            }
            Expression::TemplateLiteral(t) => {
                for expr in &t.expressions {
                    self.collect_uses_from_expr(expr);
                }
            }
            Expression::TaggedTemplateExpression(t) => {
                self.collect_uses_from_expr(&t.tag);
                for expr in &t.quasi.expressions {
                    self.collect_uses_from_expr(expr);
                }
            }
            Expression::ChainExpression(c) => {
                match &c.expression {
                    ChainElement::CallExpression(call) => {
                        self.collect_uses_from_expr(&call.callee);
                        for arg in &call.arguments {
                            self.collect_uses_from_argument(arg);
                        }
                    }
                    ChainElement::StaticMemberExpression(m) => {
                        if is_this(&m.object) {
                            let name = format!("this.{}", m.property.name);
                            self.emit_use(name, UseKind::MemberRead, m.span.start, m.span.end);
                        } else {
                            self.collect_uses_from_expr(&m.object);
                        }
                    }
                    ChainElement::ComputedMemberExpression(m) => {
                        self.collect_uses_from_expr(&m.object);
                        self.collect_uses_from_expr(&m.expression);
                    }
                    ChainElement::PrivateFieldExpression(p) => {
                        self.collect_uses_from_expr(&p.object);
                    }
                    _ => {}
                }
            }
            // TS type assertion wrappers — unwrap
            Expression::TSAsExpression(e) => {
                self.collect_uses_from_expr(&e.expression);
            }
            Expression::TSSatisfiesExpression(e) => {
                self.collect_uses_from_expr(&e.expression);
            }
            Expression::TSNonNullExpression(e) => {
                self.collect_uses_from_expr(&e.expression);
            }
            Expression::TSTypeAssertion(e) => {
                self.collect_uses_from_expr(&e.expression);
            }
            // Boundaries — do not recurse into nested functions/classes
            Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::ClassExpression(_) => {}
            // Literals, ThisExpression, etc. — no uses to collect
            _ => {}
        }
    }

    fn collect_uses_from_argument(&mut self, arg: &Argument<'_>) {
        match arg {
            Argument::SpreadElement(s) => {
                self.collect_uses_from_expr(&s.argument);
            }
            _ => {
                self.collect_uses_from_expr(arg.to_expression());
            }
        }
    }

    fn collect_uses_from_array_element(&mut self, elem: &ArrayExpressionElement<'_>) {
        match elem {
            ArrayExpressionElement::SpreadElement(s) => {
                self.collect_uses_from_expr(&s.argument);
            }
            ArrayExpressionElement::Elision(_) => {}
            _ => {
                self.collect_uses_from_expr(elem.to_expression());
            }
        }
    }

    fn collect_uses_from_property_key(&mut self, key: &PropertyKey<'_>) {
        match key {
            PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => {}
            _ => {
                self.collect_uses_from_expr(key.to_expression());
            }
        }
    }

    fn handle_assignment_expr(&mut self, a: &AssignmentExpression<'_>) {
        // RHS first
        self.collect_uses_from_expr(&a.right);

        let is_simple = a.operator == AssignmentOperator::Assign;

        // Try simple identifier target
        if let Some((name, span_start, span_end)) = extract_simple_target(&a.left) {
            if !is_simple {
                // Compound: read + write
                self.emit_use(name.clone(), UseKind::Read, span_start, span_end);
            }
            self.emit_def(name, DefKind::Assignment, span_start, span_end);
            return;
        }

        // Try this.field target
        if let Some((name, span_start, span_end)) = extract_this_field_target(&a.left) {
            if !is_simple {
                self.emit_use(name.clone(), UseKind::MemberRead, span_start, span_end);
            }
            self.emit_def(name, DefKind::Assignment, span_start, span_end);
            return;
        }

        // Destructuring assignment: ({ a } = obj) or [a] = xs
        if is_simple {
            let names = collect_assignment_target_names(&a.left);
            if !names.is_empty() {
                for (name, span_start, span_end) in names {
                    self.emit_def(name, DefKind::Destructuring, span_start, span_end);
                }
                return;
            }
        }

        // For other LHS (computed member, etc.), just walk LHS for reads
        self.collect_uses_from_assignment_target(&a.left);
    }

    fn handle_update_expr(&mut self, u: &UpdateExpression<'_>) {
        match &u.argument {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                let name = id.name.to_string();
                self.emit_use(name.clone(), UseKind::Read, u.span.start, u.span.end);
                self.emit_def(name, DefKind::Assignment, u.span.start, u.span.end);
            }
            SimpleAssignmentTarget::StaticMemberExpression(m) if is_this(&m.object) => {
                let name = format!("this.{}", m.property.name);
                self.emit_use(name.clone(), UseKind::MemberRead, u.span.start, u.span.end);
                self.emit_def(name, DefKind::Assignment, u.span.start, u.span.end);
            }
            _ => {
                // Computed member, etc. — walk the expression parts for reads
                self.collect_uses_from_simple_target(&u.argument);
            }
        }
    }

    fn collect_uses_from_assignment_target(&mut self, target: &AssignmentTarget<'_>) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(_) => {
                // Already handled in caller
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                self.collect_uses_from_expr(&m.object);
            }
            AssignmentTarget::ComputedMemberExpression(m) => {
                self.collect_uses_from_expr(&m.object);
                self.collect_uses_from_expr(&m.expression);
            }
            _ => {}
        }
    }

    fn collect_uses_from_simple_target(&mut self, target: &SimpleAssignmentTarget<'_>) {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => {}
            SimpleAssignmentTarget::StaticMemberExpression(m) => {
                self.collect_uses_from_expr(&m.object);
            }
            SimpleAssignmentTarget::ComputedMemberExpression(m) => {
                self.collect_uses_from_expr(&m.object);
                self.collect_uses_from_expr(&m.expression);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_this(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::ThisExpression(_))
}

fn is_builtin_global(name: &str) -> bool {
    matches!(
        name,
        "undefined" | "null" | "true" | "false" | "NaN" | "Infinity" | "globalThis"
    )
}

fn extract_simple_target(target: &AssignmentTarget<'_>) -> Option<(String, u32, u32)> {
    if let AssignmentTarget::AssignmentTargetIdentifier(id) = target {
        Some((id.name.to_string(), id.span.start, id.span.end))
    } else {
        None
    }
}

fn extract_this_field_target(target: &AssignmentTarget<'_>) -> Option<(String, u32, u32)> {
    if let AssignmentTarget::StaticMemberExpression(m) = target {
        if is_this(&m.object) {
            return Some((
                format!("this.{}", m.property.name),
                m.span.start,
                m.span.end,
            ));
        }
    }
    None
}

fn collect_binding_names(pattern: &BindingPattern<'_>) -> Vec<(String, u32, u32)> {
    let mut names = Vec::new();
    collect_binding_names_inner(pattern, &mut names);
    names
}

fn collect_binding_names_inner(pattern: &BindingPattern<'_>, out: &mut Vec<(String, u32, u32)>) {
    match pattern {
        BindingPattern::BindingIdentifier(id) => {
            out.push((id.name.to_string(), id.span.start, id.span.end));
        }
        BindingPattern::ObjectPattern(obj) => {
            for prop in &obj.properties {
                collect_binding_names_inner(&prop.value, out);
            }
            if let Some(ref rest) = obj.rest {
                collect_binding_names_inner(&rest.argument, out);
            }
        }
        BindingPattern::ArrayPattern(arr) => {
            for elem in &arr.elements {
                if let Some(ref pat) = elem {
                    collect_binding_names_inner(pat, out);
                }
            }
            if let Some(ref rest) = arr.rest {
                collect_binding_names_inner(&rest.argument, out);
            }
        }
        BindingPattern::AssignmentPattern(a) => {
            collect_binding_names_inner(&a.left, out);
        }
    }
}

/// Collect identifier names from an AssignmentTarget (for destructuring assignments).
fn collect_assignment_target_names(target: &AssignmentTarget<'_>) -> Vec<(String, u32, u32)> {
    let mut names = Vec::new();
    collect_assignment_target_names_inner(target, &mut names);
    names
}

fn collect_assignment_target_names_inner(
    target: &AssignmentTarget<'_>,
    out: &mut Vec<(String, u32, u32)>,
) {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            out.push((id.name.to_string(), id.span.start, id.span.end));
        }
        AssignmentTarget::ObjectAssignmentTarget(obj) => {
            collect_assignment_target_names_from_object(obj)
                .into_iter()
                .for_each(|n| out.push(n));
        }
        AssignmentTarget::ArrayAssignmentTarget(arr) => {
            collect_assignment_target_names_from_array(arr)
                .into_iter()
                .for_each(|n| out.push(n));
        }
        _ => {}
    }
}

fn collect_assignment_target_names_from_object(
    obj: &ObjectAssignmentTarget<'_>,
) -> Vec<(String, u32, u32)> {
    let mut names = Vec::new();
    for prop in &obj.properties {
        match prop {
            AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(p) => {
                names.push((
                    p.binding.name.to_string(),
                    p.binding.span.start,
                    p.binding.span.end,
                ));
            }
            AssignmentTargetProperty::AssignmentTargetPropertyProperty(p) => {
                collect_assignment_target_maybe_default_names(&p.binding, &mut names);
            }
        }
    }
    if let Some(ref rest) = obj.rest {
        collect_assignment_target_names_inner(&rest.target, &mut names);
    }
    names
}

fn collect_assignment_target_names_from_array(
    arr: &ArrayAssignmentTarget<'_>,
) -> Vec<(String, u32, u32)> {
    let mut names = Vec::new();
    for elem in &arr.elements {
        if let Some(ref el) = elem {
            collect_assignment_target_maybe_default_names(el, &mut names);
        }
    }
    if let Some(ref rest) = arr.rest {
        collect_assignment_target_names_inner(&rest.target, &mut names);
    }
    names
}

fn collect_assignment_target_maybe_default_names(
    target: &AssignmentTargetMaybeDefault<'_>,
    out: &mut Vec<(String, u32, u32)>,
) {
    match target {
        AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(d) => {
            collect_assignment_target_names_inner(&d.binding, out);
        }
        _ => {
            // Inherits AssignmentTarget variants — use to_assignment_target()
            let at = target.to_assignment_target();
            collect_assignment_target_names_inner(at, out);
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn analyze_data_flow(
    stmts: &[Statement<'_>],
    params: &FormalParameters<'_>,
    source: &str,
) -> DataFlowReport {
    let mut walker = FlowWalker::new(source);

    // 1. Seed parameters
    walker.seed_params(params);

    // 2. Hoist function declarations (body-level only)
    walker.hoist_function_decls(stmts);

    // 3. Hoist var declarations
    walker.hoist_vars(stmts);

    // 4. Walk statements
    walker.walk_stmts(stmts);

    DataFlowReport {
        defs: walker.defs,
        uses: walker.uses,
        def_use_edges: walker.edges,
    }
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_parser::Parser as OxcParser;
    use oxc_span::SourceType;

    use super::*;
    use crate::ast::collect_functions::{collect_functions, FunctionNode};

    fn analyze_named_function(source: &str, name: &str) -> DataFlowReport {
        let allocator = Allocator::default();
        let ret = OxcParser::new(&allocator, source, SourceType::ts()).parse();
        assert!(
            ret.errors.is_empty(),
            "parse errors: {:?}",
            ret.errors
        );

        let collected = collect_functions(&ret.program, source);
        let func = collected
            .iter()
            .find(|f| f.symbol_name == name)
            .unwrap_or_else(|| panic!("function `{name}` not found"));

        match &func.node {
            FunctionNode::Function(f) => analyze_data_flow(
                &f.body.as_ref().expect("expected function body").statements,
                &f.params,
                source,
            ),
            FunctionNode::Arrow(a) => analyze_data_flow(&a.body.statements, &a.params, source),
            FunctionNode::Class(_) => panic!("expected function, found class"),
        }
    }

    #[test]
    fn block_level_function_shadows_same_named_body_level_function() {
        let source = r#"function outer(c: boolean) {
  function x() {}
  if (c) {
    function x() {}
    return x
  }
  return x
}
"#;

        let df = analyze_named_function(source, "outer");

        let x_defs: Vec<&Def> = df.defs.iter().filter(|d| d.name == "x").collect();
        assert_eq!(x_defs.len(), 2, "expected outer and inner x defs: {x_defs:?}");

        let inner_return_edges: Vec<&DefUseEdge> = df
            .def_use_edges
            .iter()
            .filter(|e| e.def_name == "x" && e.use_line == 5)
            .collect();
        assert_eq!(inner_return_edges.len(), 1, "inner return edges: {inner_return_edges:?}");
        assert_eq!(inner_return_edges[0].def_line, 4);

        let outer_return_edges: Vec<&DefUseEdge> = df
            .def_use_edges
            .iter()
            .filter(|e| e.def_name == "x" && e.use_line == 7)
            .collect();
        assert_eq!(outer_return_edges.len(), 1, "outer return edges: {outer_return_edges:?}");
        assert_eq!(outer_return_edges[0].def_line, 2);
    }
}
