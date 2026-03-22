use super::model::{CallCategory, CallKind, ResolvedTarget};

/// Well-known builtin / collection / utility methods that should not clutter the call graph.
const BUILTIN_METHODS: &[&str] = &[
    // Array / collection
    "map", "filter", "reduce", "forEach", "some", "every", "find", "findIndex",
    "includes", "indexOf", "lastIndexOf", "flat", "flatMap", "fill", "copyWithin",
    "sort", "reverse", "splice", "slice", "concat", "join", "entries", "keys", "values",
    "push", "pop", "shift", "unshift", "at",
    // String
    "toString", "toLocaleString", "toLowerCase", "toUpperCase", "trim", "trimStart",
    "trimEnd", "padStart", "padEnd", "startsWith", "endsWith", "charAt", "charCodeAt",
    "split", "replace", "replaceAll", "match", "matchAll", "search", "substring",
    "repeat", "normalize",
    // Object
    "hasOwnProperty", "valueOf", "toJSON",
    // Number / Math
    "toFixed", "toPrecision",
    // Promise
    "then", "catch", "finally",
    // Console / logging (informational, not domain logic)
    "log", "warn", "error", "info", "debug", "trace",
    // JSON
    "parse", "stringify",
];

/// Infra keywords in receiver chains that suggest infrastructure calls.
const INFRA_KEYWORDS: &[&str] = &[
    "repo", "repository", "client", "http", "axios", "api",
    "db", "database", "store", "cache", "queue", "notifier",
    "mailer", "sender", "publisher", "emitter",
];

pub fn classify_call(
    kind: &CallKind,
    target_name: &str,
    receiver: Option<&str>,
    resolved: Option<&ResolvedTarget>,
) -> CallCategory {
    // Resolved internal calls are domain
    if let Some(ResolvedTarget::Internal(_)) = resolved {
        return CallCategory::Domain;
    }

    // Resolved imports are domain (external but intentional)
    if let Some(ResolvedTarget::Import(_)) = resolved {
        return CallCategory::Domain;
    }

    // Check if it's a builtin method
    if is_builtin(target_name) {
        return CallCategory::Builtin;
    }

    // Check if receiver suggests infra
    if let Some(recv) = receiver {
        let lower_recv = recv.to_lowercase();
        for kw in INFRA_KEYWORDS {
            if lower_recv.contains(kw) {
                return CallCategory::Infra;
            }
        }
    }

    // this.method() that wasn't resolved internally — likely infra via injected dependency
    if matches!(kind, CallKind::MemberCall) {
        if let Some(recv) = receiver {
            if recv.starts_with("this.") {
                return CallCategory::Infra;
            }
        }
    }

    CallCategory::Unresolved
}

fn is_builtin(name: &str) -> bool {
    BUILTIN_METHODS.contains(&name)
}
