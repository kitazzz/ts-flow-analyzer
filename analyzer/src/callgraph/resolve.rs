use std::collections::HashMap;

use crate::ast::collect_functions::CollectedFunction;

use super::model::{CallKind, CallSite, ImportEntry, ResolvedTarget};

pub fn resolve_call(
    call: &CallSite,
    caller_class: Option<&str>,
    symbols: &[CollectedFunction<'_>],
    imports: &[ImportEntry],
    hierarchy: &HashMap<String, Option<String>>,
) -> Option<ResolvedTarget> {
    match call.kind {
        CallKind::ThisMethod => {
            let class_name = caller_class?;
            // Walk the inheritance chain: current class → parent → grandparent ...
            let mut current = Some(class_name.to_string());
            while let Some(cls) = current {
                let target = format!("{}#{}", cls, call.target_name);
                if let Some(s) = symbols.iter().find(|s| s.symbol_name == target) {
                    return Some(ResolvedTarget::Internal(s.symbol_name.clone()));
                }
                current = hierarchy.get(&cls).and_then(|p| p.clone());
            }
            None
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
            let class_name = caller_class?;
            // super starts from the parent class, not the caller's own class
            let mut current = hierarchy.get(class_name)?.clone();
            while let Some(cls) = current {
                let target = format!("{}#{}", cls, call.target_name);
                if let Some(s) = symbols.iter().find(|s| s.symbol_name == target) {
                    return Some(ResolvedTarget::Internal(s.symbol_name.clone()));
                }
                current = hierarchy.get(&cls).and_then(|p| p.clone());
            }
            None
        }
        CallKind::SuperConstructor => {
            let class_name = caller_class?;
            // Known limitation: super() resolves only to explicit or synthesized constructors.
            // Implicit default parent constructors (classes with no constructor and no field initializers)
            // are not yet modeled. See follow-up if needed.
            let parent = hierarchy.get(class_name)?.as_ref()?;
            let target = format!("{}#constructor", parent);
            if let Some(s) = symbols.iter().find(|s| s.symbol_name == target) {
                return Some(ResolvedTarget::Internal(s.symbol_name.clone()));
            }
            None
        }
        CallKind::New => {
            // new Foo() — try file-level class first, then file-level function, then import
            if let Some(s) = symbols.iter().find(|s| {
                s.symbol_name == call.target_name && s.class_name.is_some()
            }) {
                return Some(ResolvedTarget::Internal(s.symbol_name.clone()));
            }
            // Also resolve constructor functions (file-level non-class functions)
            if let Some(s) = symbols.iter().find(|s| {
                s.member_name.as_deref() == Some(call.target_name.as_str())
                    && s.class_name.is_none()
            }) {
                return Some(ResolvedTarget::Internal(s.symbol_name.clone()));
            }
            if let Some(imp) = imports.iter().find(|i| i.local_name == call.target_name) {
                return Some(ResolvedTarget::Import(imp.clone()));
            }
            None
        }
        CallKind::MemberCall => {
            // Unresolved — we record receiver chain for informational purposes
            None
        }
    }
}
