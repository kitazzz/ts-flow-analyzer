use oxc_semantic::SemanticBuilder;
use oxc_ast::ast::Program;

pub struct CfgContext<'a> {
    pub semantic: oxc_semantic::Semantic<'a>,
}

pub fn build_cfg_context<'a>(program: &'a Program<'a>) -> CfgContext<'a> {
    let result = SemanticBuilder::new()
        .with_cfg(true)
        .build(program);
    if !result.errors.is_empty() {
        for err in &result.errors {
            eprintln!("CFG semantic warning: {:?}", err);
        }
    }
    CfgContext { semantic: result.semantic }
}
