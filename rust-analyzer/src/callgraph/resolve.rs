use crate::ast::collect_functions::CollectedFunction;

use super::model::{CallKind, CallSite, ImportEntry, ResolvedTarget};

pub fn resolve_call(
    call: &CallSite,
    caller_class: Option<&str>,
    symbols: &[CollectedFunction<'_>],
    imports: &[ImportEntry],
) -> Option<ResolvedTarget> {
    match call.kind {
        CallKind::ThisMethod => {
            let class_name = caller_class?;
            let target = format!("{}#{}", class_name, call.target_name);
            symbols
                .iter()
                .find(|s| s.symbol_name == target)
                .map(|s| ResolvedTarget::Internal(s.symbol_name.clone()))
        }
        CallKind::Direct => {
            // Try file-level function first
            if let Some(s) = symbols.iter().find(|s| {
                s.member_name.as_deref() == Some(call.target_name.as_str())
                    && s.class_name.is_none()
            }) {
                return Some(ResolvedTarget::Internal(s.symbol_name.clone()));
            }
            // Try import
            if let Some(imp) = imports.iter().find(|i| i.local_name == call.target_name) {
                return Some(ResolvedTarget::Import(imp.clone()));
            }
            None
        }
        CallKind::Super => {
            // Deferred: requires parent class resolution
            None
        }
        CallKind::MemberCall => {
            // Unresolved — we record receiver chain for informational purposes
            None
        }
    }
}
