mod ast;
mod callgraph;
mod cli;
mod config;
mod control_flow;
mod dataflow;
mod decision;
mod effects;
mod ir;
mod metrics;
mod model;
mod parser;
mod predicates;

use clap::Parser;
use oxc_allocator::Allocator;
use oxc_parser::Parser as OxcParser;
use oxc_span::SourceType;
use std::path::Path;

use ast::collect_functions::{collect_functions, FunctionNode};
use callgraph::build_call_graph;
use cli::Cli;
use config::load_config;
use decision::table::build_decision_table;
use effects::extract::{extract_effects, extract_field_initializer_effects, extract_parameter_property_effects};
use metrics::complexity::analyze_basic_complexity;
use metrics::cyclomatic::analyze_cyclomatic_complexity;
use metrics::function_nesting::analyze_function_nesting;
use metrics::nesting::analyze_max_nesting_depth;
use model::{Effect, EffectKind, FunctionMetrics, FunctionReport, SideEffectClass, SymbolKind};
use dataflow::analyze::analyze_data_flow;
use predicates::extract::extract_predicates;
use predicates::normalize::normalize_predicates;

fn main() {
    let cli = Cli::parse();
    let config = match load_config(cli.config.as_deref()) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    let path = Path::new(&cli.file);
    let source = match parser::load_source::load_source(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let include_predicates = cli.all || cli.predicates;
    let include_effects = cli.all || cli.effects;
    let include_decision = cli.all || cli.decision;
    let include_data_flow = cli.all || cli.data_flow;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_default();
    let ret = OxcParser::new(&allocator, &source, source_type).parse();

    if !ret.errors.is_empty() {
        for err in &ret.errors {
            eprintln!("Parse warning: {:?}", err);
        }
    }

    let file_path = path.to_string_lossy().to_string();
    let collected = collect_functions(&ret.program, &source);

    // Build CFG context when needed (cfg-analysis feature + --decision-enhanced, --cfg-dot, or --graph)
    let _need_cfg = cli.decision_enhanced || cli.cfg_dot.is_some()
        || cli.graph || cli.graph_dot.is_some();

    #[cfg(feature = "cfg-analysis")]
    let cfg_ctx = _need_cfg.then(|| control_flow::build_cfg_context(&ret.program));

    // Handle --cfg-dot: validate function name exists, output DOT, and exit
    if let Some(ref fn_name) = cli.cfg_dot {
        let target = collected.iter().find(|f| &f.symbol_name == fn_name);
        let Some(_target) = target else {
            let available: Vec<&str> = collected.iter().map(|f| f.symbol_name.as_str()).collect();
            eprintln!(
                "error: function '{}' not found in file. Available: {}",
                fn_name,
                available.join(", ")
            );
            std::process::exit(1);
        };
        #[cfg(feature = "cfg-analysis")]
        {
            if matches!(_target.node, FunctionNode::Class(_)) {
                eprintln!(
                    "error: --cfg-dot does not support class symbols; choose a function or method"
                );
                std::process::exit(1);
            }
            let dot = cfg_ctx
                .as_ref()
                .and_then(|ctx| control_flow::render_cfg_dot(ctx, &_target.node))
                .unwrap_or_else(|| {
                    eprintln!("CFG build failed");
                    std::process::exit(1)
                });
            println!("{dot}");
            return;
        }
        #[cfg(not(feature = "cfg-analysis"))]
        {
            eprintln!("--cfg-dot requires building with --features cfg-analysis");
            std::process::exit(1);
        }
    }

    // Handle --call-graph-dot: build call graph, render DOT, and exit
    if let Some(ref entrypoint) = cli.call_graph_dot {
        let graph = build_call_graph(&collected, &ret.program, &source, cli.include_builtin_calls);
        // Validate the entrypoint exists
        if !entrypoint.is_empty() && !graph.nodes.iter().any(|n| n.symbol_name == *entrypoint) {
            let available: Vec<&str> = graph.nodes.iter().map(|n| n.symbol_name.as_str()).collect();
            eprintln!(
                "error: function '{}' not found in call graph. Available: {}",
                entrypoint,
                available.join(", ")
            );
            std::process::exit(1);
        }
        let entry = if entrypoint.is_empty() {
            None
        } else {
            Some(entrypoint.as_str())
        };
        let dot = callgraph::dot::render_call_graph_dot(&graph, entry);
        println!("{dot}");
        return;
    }

    // Handle --graph-dot: build graph, render DOT, and exit
    if let Some(ref fn_name) = cli.graph_dot {
        let target = collected.iter().find(|f| &f.symbol_name == fn_name);
        let Some(_target) = target else {
            let available: Vec<&str> = collected.iter().map(|f| f.symbol_name.as_str()).collect();
            eprintln!(
                "error: function '{}' not found in file. Available: {}",
                fn_name,
                available.join(", ")
            );
            std::process::exit(1);
        };
        // Build graph for the whole file, then render DOT filtered to the function
        let mut builder = ir::builder::GraphBuilder::new();
        let symbol_map = ir::from_functions::functions_to_graph(&collected, &mut builder);
        let cg = build_call_graph(&collected, &ret.program, &source, cli.include_builtin_calls);
        ir::from_callgraph::callgraph_to_graph(&cg, &symbol_map, &mut builder);
        // Decision tables (gated by --decision / --all)
        if include_decision {
        for func in &collected {
            if matches!(func.node, FunctionNode::Class(_)) { continue; }
            if let Some(dt) = build_decision_table(
                &func.node, &func.symbol_name, &func.symbol_kind.to_string(),
                &source, &config.decision_table, cli.decision_enhanced,
                #[cfg(feature = "cfg-analysis")]
                cfg_ctx.as_ref().filter(|_| cli.decision_enhanced),
            ) {
                if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                    ir::from_decision::decision_to_graph(&dt, parent_id, &mut builder);
                }
            }
        }
        }
        #[cfg(feature = "cfg-analysis")]
        {
            if let Some(ref ctx) = cfg_ctx {
                for func in &collected {
                    if matches!(func.node, FunctionNode::Class(_)) { continue; }
                    if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                        ir::from_cfg::cfg_to_graph(ctx, &func.node, parent_id, &mut builder);
                    }
                }
            }
        }
        // Data-flow → Graph IR
        if include_data_flow {
            for func in &collected {
                if matches!(func.node, FunctionNode::Class(_)) { continue; }
                if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                    let df_report = match &func.node {
                        FunctionNode::Function(f) => {
                            f.body.as_ref().map(|b| analyze_data_flow(&b.statements, &f.params, &source))
                        }
                        FunctionNode::Arrow(a) => {
                            Some(analyze_data_flow(&a.body.statements, &a.params, &source))
                        }
                        _ => None,
                    };
                    if let Some(report) = df_report {
                        ir::from_data_flow::data_flow_to_graph(&report, parent_id, &mut builder);
                    }
                }
            }
        }
        let graph_ir = builder.build(file_path);
        let dot = ir::dot::render_graph_dot(&graph_ir, fn_name);
        println!("{dot}");
        return;
    }

    let include_call_graph = cli.all || cli.call_graph;

    let mut reports: Vec<FunctionReport> = Vec::new();

    for func in &collected {
        // Apply function filter
        if let Some(filter) = &cli.function {
            if &func.symbol_name != filter {
                continue;
            }
        }

        let stmts = get_statements(&func.node);

        let metrics = if cli.metrics || stmts.is_some() {
            compute_metrics(stmts.unwrap_or(&[]))
        } else {
            FunctionMetrics::default()
        };

        let predicates = if include_predicates {
            stmts.map(|s| {
                let raw = extract_predicates(s, &source);
                normalize_predicates(raw)
            })
        } else {
            None
        };

        let effects = if include_effects {
            let mut body_effs = stmts
                .map(|s| extract_effects(s, &source))
                .unwrap_or_default();

            if func.member_name.as_deref() == Some("constructor") {
                // Build synthetic effects from field initializers + parameter properties
                let mut init_effs = Vec::new();
                init_effs.extend(extract_field_initializer_effects(
                    &func.field_initializers,
                    &source,
                ));
                if let FunctionNode::Function(f) = &func.node {
                    init_effs.extend(extract_parameter_property_effects(f, &source));
                }

                let is_derived = func.parent_class.is_some();

                if is_derived {
                    if func.has_implicit_super {
                        // Synthetic constructor (no explicit body): super() → init_effs
                        let mut combined = Vec::new();
                        combined.push(Effect {
                            kind: EffectKind::Call,
                            side_effect: SideEffectClass::PureCall,
                            text: "super() /* implicit */".to_string(),
                            line: func.start_line,
                        });
                        combined.extend(init_effs);
                        body_effs = combined;
                    } else if !init_effs.is_empty() {
                        // Explicit constructor: insert init_effs after super() call
                        let super_pos = body_effs.iter().position(|e| {
                            e.kind == EffectKind::Call
                                && e.text.trim_start().starts_with("super(")
                        });
                        let insert_at = super_pos.map(|p| p + 1).unwrap_or(0);
                        let mut combined = body_effs[..insert_at].to_vec();
                        combined.extend(init_effs);
                        combined.extend_from_slice(&body_effs[insert_at..]);
                        body_effs = combined;
                    }
                } else if !init_effs.is_empty() {
                    // Non-derived class: prepend init effects before body
                    let mut combined = init_effs;
                    combined.append(&mut body_effs);
                    body_effs = combined;
                }
            }

            if body_effs.is_empty() {
                None
            } else {
                Some(body_effs)
            }
        } else {
            None
        };

        let decision_table = if include_decision {
            build_decision_table(
                &func.node,
                &func.symbol_name,
                &func.symbol_kind.to_string(),
                &source,
                &config.decision_table,
                cli.decision_enhanced,
                #[cfg(feature = "cfg-analysis")]
                cfg_ctx.as_ref().filter(|_| cli.decision_enhanced),
            )
        } else {
            None
        };

        let data_flow = if include_data_flow {
            match &func.node {
                FunctionNode::Function(f) => {
                    f.body.as_ref().map(|b| {
                        analyze_data_flow(&b.statements, &f.params, &source)
                    })
                }
                FunctionNode::Arrow(a) => {
                    Some(analyze_data_flow(&a.body.statements, &a.params, &source))
                }
                FunctionNode::Class(_) => None,
            }
        } else {
            None
        };

        let function_name = func
            .member_name
            .clone()
            .unwrap_or_else(|| func.symbol_name.clone());

        reports.push(FunctionReport {
            symbol_name: func.symbol_name.clone(),
            symbol_kind: func.symbol_kind.clone(),
            class_name: func.class_name.clone(),
            member_name: func.member_name.clone(),
            function_name,
            file_path: file_path.clone(),
            start_line: func.start_line,
            metrics,
            predicates,
            effects,
            decision_table,
            data_flow,
        });
    }

    let include_graph = cli.graph;
    let output_json = cli.json || include_graph; // --graph implies --json

    if output_json {
        let mut result = serde_json::Map::new();
        let need_object_format = include_call_graph || include_graph;

        if need_object_format {
            result.insert("functions".to_string(), serde_json::json!(reports));

            if include_call_graph {
                let graph =
                    build_call_graph(&collected, &ret.program, &source, cli.include_builtin_calls);
                result.insert("callGraph".to_string(), serde_json::json!(graph));
            }

            if include_graph {
                let mut builder = ir::builder::GraphBuilder::new();
                let symbol_map = ir::from_functions::functions_to_graph(&collected, &mut builder);
                let cg = build_call_graph(&collected, &ret.program, &source, cli.include_builtin_calls);
                ir::from_callgraph::callgraph_to_graph(&cg, &symbol_map, &mut builder);

                // Decision tables (gated by --decision / --all)
                if include_decision {
                    for func in &collected {
                        if matches!(func.node, FunctionNode::Class(_)) { continue; }
                        if let Some(dt) = build_decision_table(
                            &func.node, &func.symbol_name, &func.symbol_kind.to_string(),
                            &source, &config.decision_table, cli.decision_enhanced,
                            #[cfg(feature = "cfg-analysis")]
                            cfg_ctx.as_ref().filter(|_| cli.decision_enhanced),
                        ) {
                            if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                                ir::from_decision::decision_to_graph(&dt, parent_id, &mut builder);
                            }
                        }
                    }
                }

                // CFG blocks (feature-gated)
                #[cfg(feature = "cfg-analysis")]
                {
                    if let Some(ref ctx) = cfg_ctx {
                        for func in &collected {
                            if matches!(func.node, FunctionNode::Class(_)) { continue; }
                            if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                                ir::from_cfg::cfg_to_graph(ctx, &func.node, parent_id, &mut builder);
                            }
                        }
                    }
                }

                // Data-flow → Graph IR
                if include_data_flow {
                    for func in &collected {
                        if matches!(func.node, FunctionNode::Class(_)) { continue; }
                        if let Some(&parent_id) = symbol_map.get(&func.symbol_name) {
                            let df_report = match &func.node {
                                FunctionNode::Function(f) => {
                                    f.body.as_ref().map(|b| analyze_data_flow(&b.statements, &f.params, &source))
                                }
                                FunctionNode::Arrow(a) => {
                                    Some(analyze_data_flow(&a.body.statements, &a.params, &source))
                                }
                                _ => None,
                            };
                            if let Some(report) = df_report {
                                ir::from_data_flow::data_flow_to_graph(&report, parent_id, &mut builder);
                            }
                        }
                    }
                }

                let graph_ir = builder.build(file_path.clone());
                result.insert("graph".to_string(), serde_json::json!(graph_ir));
            }

            let output = serde_json::Value::Object(result);
            match serde_json::to_string_pretty(&output) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Error serializing JSON: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            match serde_json::to_string_pretty(&serde_json::json!(reports)) {
                Ok(json) => println!("{}", json),
                Err(e) => {
                    eprintln!("Error serializing JSON: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else {
        // Human-readable output
        for report in &reports {
            println!("─── {} ({:?}) ───", report.symbol_name, report.symbol_kind);
            println!("  File: {}:{}", report.file_path, report.start_line);
            let m = &report.metrics;
            println!("  Metrics:");
            println!("    cyclomatic: {}", m.cyclomatic_complexity);
            println!(
                "    if: {}, else-if: {}, switch: {}, ternary: {}",
                m.if_count, m.else_if_count, m.switch_count, m.ternary_count
            );
            println!(
                "    returns: {}, max nesting: {}",
                m.return_count, m.max_nesting_depth
            );
            println!(
                "    local fns: {}, fn depth: {}, callback depth: {}",
                m.local_function_count, m.function_nesting_depth, m.callback_nesting_depth
            );

            if let Some(preds) = &report.predicates {
                println!("  Predicates ({}):", preds.len());
                for p in preds {
                    let neg = if p.negated { "!" } else { "" };
                    println!(
                        "    [{:?}] {}{} ({:?}) @ line {}",
                        p.context, neg, p.text, p.kind, p.line
                    );
                    if let Some(name) = &p.normalized_name {
                        println!("      -> {}", name);
                    }
                }
            }

            if let Some(effs) = &report.effects {
                println!("  Effects ({}):", effs.len());
                for e in effs {
                    println!(
                        "    {:?}/{:?} @ line {}: {}",
                        e.kind,
                        e.side_effect,
                        e.line,
                        &e.text[..e.text.len().min(60)]
                    );
                }
            }

            if let Some(df) = &report.data_flow {
                println!(
                    "  Data Flow: {} defs, {} uses, {} edges",
                    df.defs.len(),
                    df.uses.len(),
                    df.def_use_edges.len()
                );
                for edge in &df.def_use_edges {
                    let may = if edge.may_reach { " (may)" } else { "" };
                    println!(
                        "    {} @ line {} → {:?} @ line {}{}",
                        edge.def_name, edge.def_line, edge.use_kind, edge.use_line, may
                    );
                }
            }

            println!();
        }
    }
}

fn get_statements<'a>(node: &'a FunctionNode<'a>) -> Option<&'a [oxc_ast::ast::Statement<'a>]> {
    match node {
        FunctionNode::Function(f) => f.body.as_ref().map(|b| b.statements.as_slice()),
        FunctionNode::Arrow(arrow) => Some(arrow.body.statements.as_slice()),
        FunctionNode::Class(_) => None,
    }
}

fn compute_metrics(stmts: &[oxc_ast::ast::Statement<'_>]) -> FunctionMetrics {
    let basic = analyze_basic_complexity(stmts);
    let max_nesting = analyze_max_nesting_depth(stmts);
    let cc = analyze_cyclomatic_complexity(stmts);
    let fn_nesting = analyze_function_nesting(stmts);

    FunctionMetrics {
        if_count: basic.if_count,
        else_if_count: basic.else_if_count,
        switch_count: basic.switch_count,
        ternary_count: basic.ternary_count,
        return_count: basic.return_count,
        max_nesting_depth: max_nesting,
        cyclomatic_complexity: cc,
        local_function_count: fn_nesting.local_function_count,
        function_nesting_depth: fn_nesting.function_nesting_depth,
        callback_nesting_depth: fn_nesting.callback_nesting_depth,
        logical_operator_count: basic.logical_operator_count,
        negation_count: basic.negation_count,
        atomic_condition_count: basic.atomic_condition_count,
        max_condition_depth: basic.max_condition_depth,
    }
}

// Needed for SymbolKind display - ensure it's used
#[allow(dead_code)]
fn symbol_kind_str(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::VariableFunction => "variableFunction",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
    }
}
