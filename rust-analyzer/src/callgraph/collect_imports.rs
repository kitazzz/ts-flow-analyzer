use oxc_ast::ast::*;

use super::model::ImportEntry;

pub fn collect_imports(program: &Program<'_>) -> Vec<ImportEntry> {
    let mut results = Vec::new();
    for stmt in &program.body {
        if let Statement::ImportDeclaration(import) = stmt {
            // Skip type-only imports — they produce no runtime calls
            if import.import_kind.is_type() {
                continue;
            }
            let source_path = import.source.value.to_string();
            if let Some(specifiers) = &import.specifiers {
                for spec in specifiers {
                    match spec {
                        ImportDeclarationSpecifier::ImportSpecifier(s) => {
                            // Skip type-only specifiers within a value import
                            if s.import_kind.is_type() {
                                continue;
                            }
                            let imported_name = match &s.imported {
                                ModuleExportName::IdentifierName(id) => id.name.to_string(),
                                ModuleExportName::IdentifierReference(id) => id.name.to_string(),
                                ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
                            };
                            results.push(ImportEntry {
                                local_name: s.local.name.to_string(),
                                imported_name,
                                source: source_path.clone(),
                            });
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            results.push(ImportEntry {
                                local_name: s.local.name.to_string(),
                                imported_name: "default".to_string(),
                                source: source_path.clone(),
                            });
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            results.push(ImportEntry {
                                local_name: s.local.name.to_string(),
                                imported_name: "*".to_string(),
                                source: source_path.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    results
}
