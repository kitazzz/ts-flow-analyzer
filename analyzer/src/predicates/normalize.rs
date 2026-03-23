use crate::model::{AtomicPredicate, PredicateKind};

pub fn normalize_predicates(predicates: Vec<AtomicPredicate>) -> Vec<AtomicPredicate> {
    predicates
        .into_iter()
        .map(|p| match derive_names(&p) {
            Ok((name, true_meaning, false_meaning)) => AtomicPredicate {
                normalized_name: Some(name),
                true_meaning: Some(true_meaning),
                false_meaning: Some(false_meaning),
                ..p
            },
            Err(_) => p,
        })
        .collect()
}

fn derive_names(p: &AtomicPredicate) -> Result<(String, String, String), ()> {
    match p.kind {
        PredicateKind::Truthiness => Ok(derive_truthiness(&p.text, p.negated)),
        PredicateKind::NullCheck => Ok(derive_null_check(&p.text, p.negated)),
        PredicateKind::Comparison => derive_comparison(&p.text, p.negated),
        PredicateKind::Call => Ok(derive_call(&p.text, p.negated)),
        PredicateKind::Negation => {
            let stripped = strip_leading_not(&p.text);
            Ok(derive_truthiness(&stripped, !p.negated))
        }
        _ => Err(()),
    }
}

fn derive_truthiness(text: &str, negated: bool) -> (String, String, String) {
    let base = path_to_ident(text);

    // Use the raw text's last dot segment to detect boolean-ness
    let raw_leaf = last_dot_segment(
        text.trim()
            .trim_start_matches("await ")
            .trim_start_matches("this."),
    );

    // Category C: already-meaningful boolean names (has*, is*, can*, should*, etc.)
    // Use as-is without appending Exists/Missing
    if is_boolean_named(&raw_leaf) {
        let leaf = raw_leaf.clone();
        return if negated {
            let neg_name = negate_boolean_name(&base, &leaf);
            (
                neg_name,
                format!("{} is false", text),
                format!("{} is true", text),
            )
        } else {
            (
                base.clone(),
                format!("{} is true", text),
                format!("{} is false", text),
            )
        };
    }

    // Category B: boolean property access (e.g. input.forceApprove, riskCheck.ok)
    // These are boolean values, not nullable objects
    if text.contains('.') && is_boolean_leaf(&raw_leaf) {
        {
            let leaf = raw_leaf.clone();
            return if negated {
                let neg_name = negate_boolean_name(&base, &leaf);
                (
                    neg_name,
                    format!("{} is false", text),
                    format!("{} is true", text),
                )
            } else {
                (
                    base.clone(),
                    format!("{} is true", text),
                    format!("{} is false", text),
                )
            };
        }
    }

    // Category A: object/nullable - plain identifiers that represent nullable objects
    if negated {
        (
            format!("{}Missing", base),
            format!("{} is missing / falsy", text),
            format!("{} exists", text),
        )
    } else {
        (
            format!("{}Exists", base),
            format!("{} exists", text),
            format!("{} is missing / falsy", text),
        )
    }
}

/// Check if a name is an already-meaningful boolean (has*, is*, can*, should*, etc.)
fn is_boolean_named(name: &str) -> bool {
    let prefixes = [
        "has", "is", "can", "should", "will", "did", "was", "are", "does", "do", "needs", "allows",
        "requires", "includes", "contains", "exists", "matches", "supports", "enables",
    ];
    let lower = name.to_lowercase();
    for prefix in &prefixes {
        if lower.starts_with(prefix) {
            // Must be followed by uppercase or end of string (camelCase boundary)
            let rest = &name[prefix.len()..];
            if rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_uppercase()) {
                return true;
            }
        }
    }
    false
}

/// Check if a leaf property name suggests a boolean value
fn is_boolean_leaf(name: &str) -> bool {
    // Known boolean property names
    let boolean_props = [
        "ok",
        "valid",
        "enabled",
        "disabled",
        "active",
        "inactive",
        "visible",
        "hidden",
        "locked",
        "unlocked",
        "done",
        "ready",
        "success",
        "failed",
        "approved",
        "rejected",
        "confirmed",
        "verified",
        "authenticated",
        "authorized",
        "completed",
    ];
    let lower = name.to_lowercase();
    if boolean_props.contains(&lower.as_str()) {
        return true;
    }
    // Also treat boolean-named leaves as boolean
    if is_boolean_named(name) {
        return true;
    }
    // Boolean-action suffixes: forceApprove, skipValidation, allowOverride, etc.
    let boolean_prefixes = [
        "force", "skip", "allow", "prevent", "disable", "enable", "ignore",
    ];
    for prefix in &boolean_prefixes {
        if lower.starts_with(prefix) {
            let rest = &name[prefix.len()..];
            if rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_uppercase()) {
                return true;
            }
        }
    }
    false
}

/// Check if a property access suggests a boolean property
fn is_boolean_property(name: &str) -> bool {
    is_boolean_leaf(name) || is_boolean_named(name)
}

/// Negate a boolean name meaningfully
fn negate_boolean_name(full: &str, leaf: &str) -> String {
    let lower_leaf = leaf.to_lowercase();

    // Find where the leaf starts in the full name (case-insensitive suffix match)
    let prefix = if full.len() > leaf.len() && full.to_lowercase().ends_with(&lower_leaf) {
        &full[..full.len() - leaf.len()]
    } else {
        ""
    };

    // Special cases for common patterns
    if lower_leaf == "ok" {
        return format!("{}NotOk", prefix);
    }
    if lower_leaf == "valid" {
        return format!("{}Invalid", prefix);
    }
    if lower_leaf == "enabled" {
        return format!("{}Disabled", prefix);
    }
    if lower_leaf == "active" {
        return format!("{}Inactive", prefix);
    }
    if lower_leaf == "success" {
        return format!("{}Failed", prefix);
    }

    // has* -> no* (hasItems -> noItems, hasZeroPriceItem -> noZeroPriceItem)
    if lower_leaf.starts_with("has") && leaf.len() > 3 {
        let rest = &leaf[3..];
        return format!("{}no{}", prefix, rest);
    }

    // is* -> isNot*
    if lower_leaf.starts_with("is") && leaf.len() > 2 {
        let rest = &leaf[2..];
        return format!("{}isNot{}", prefix, rest);
    }

    // can* -> cannot*
    if lower_leaf.starts_with("can") && leaf.len() > 3 {
        let rest = &leaf[3..];
        return format!("{}cannot{}", prefix, rest);
    }

    // should* -> shouldNot*
    if lower_leaf.starts_with("should") && leaf.len() > 6 {
        let rest = &leaf[6..];
        return format!("{}shouldNot{}", prefix, rest);
    }

    // force* / skip* / allow* -> not + original (forceApprove -> forceApproveDisabled)
    let action_prefixes = ["force", "skip", "allow", "prevent", "ignore"];
    for ap in &action_prefixes {
        if lower_leaf.starts_with(ap) && leaf.len() > ap.len() {
            return format!("{}Disabled", full);
        }
    }

    // Generic boolean: append Disabled
    format!("{}Disabled", full)
}

fn derive_null_check(text: &str, negated: bool) -> (String, String, String) {
    // e.g. "order === null", "user !== undefined"
    let re_match = parse_null_check(text);
    let (subject, is_null_op) = match re_match {
        Some((subj, op)) => {
            let is_null = op == "===" || op == "==";
            (subj, is_null)
        }
        None => (text.to_string(), true),
    };
    let base = path_to_ident(&subject);

    // negated + isNullOp: `!(order === null)` → orderNotNull
    let result_negated = if negated { !is_null_op } else { is_null_op };
    if result_negated {
        (
            format!("{}Null", base),
            format!("{} is null/undefined", subject),
            format!("{} is not null", subject),
        )
    } else {
        (
            format!("{}NotNull", base),
            format!("{} is not null", subject),
            format!("{} is null/undefined", subject),
        )
    }
}

fn parse_null_check(text: &str) -> Option<(String, String)> {
    // Match: subject OP null/undefined
    let operators = ["===", "!==", "==", "!="];
    for op in &operators {
        if let Some(pos) = text.find(op) {
            let left = text[..pos].trim().to_string();
            let right = text[pos + op.len()..].trim();
            if right == "null" || right == "undefined" {
                return Some((left, op.to_string()));
            }
        }
    }
    None
}

fn derive_comparison(text: &str, negated: bool) -> Result<(String, String, String), ()> {
    // Parse: left OP right
    let (left, op, right) = parse_comparison(text).ok_or(())?;

    // Role / status string literal comparison
    if let Some(string_val) = parse_string_literal(&right) {
        let base = path_to_ident(&left);
        let value_id = capitalize(&camelize(&string_val));
        let not_op = op == "!==" || op == "!=";
        let effective_not = if negated { !not_op } else { not_op };
        if effective_not {
            return Ok((
                format!("{}Not{}", base, value_id),
                format!("{} is not '{}'", left, string_val),
                format!("{} is '{}'", left, string_val),
            ));
        }
        return Ok((
            format!("{}Is{}", base, value_id),
            format!("{} is '{}'", left, string_val),
            format!("{} is not '{}'", left, string_val),
        ));
    }

    // .length comparison
    if left.ends_with(".length") {
        let subject = path_to_ident(&left.replace(".length", ""));
        let gt_op = op == ">" || op == ">=";
        let effective_gt = if negated { !gt_op } else { gt_op };
        if right == "0" {
            return Ok(if effective_gt {
                (
                    format!("{}NonEmpty", subject),
                    format!("{} > 0", left),
                    format!("{} is 0", left),
                )
            } else {
                (
                    format!("{}Empty", subject),
                    format!("{} is 0", left),
                    format!("{} > 0", left),
                )
            });
        }
        let right_camel = camelize(&right);
        return Ok(if effective_gt {
            (
                format!("{}LengthAbove{}", subject, right_camel),
                format!("{} > {}", left, right),
                format!("{} <= {}", left, right),
            )
        } else {
            (
                format!("{}LengthBelow{}", subject, right_camel),
                format!("{} <= {}", left, right),
                format!("{} > {}", left, right),
            )
        });
    }

    // Numeric threshold comparison
    let is_numeric_rhs = right.chars().all(|c| c.is_ascii_digit() || c == '.')
        || right.chars().all(|c| c.is_ascii_uppercase() || c == '_');

    if is_numeric_rhs {
        let base = path_to_ident(&left);
        let eop = if negated { negate_op(&op) } else { op.clone() };
        let is_gt = eop == ">" || eop == ">=";
        let is_strict = eop == ">" || eop == "<";

        if right == "0" {
            return Ok(if is_gt {
                if is_strict {
                    (
                        format!("{}Positive", base),
                        format!("{} > 0", left),
                        format!("{} <= 0", left),
                    )
                } else {
                    (
                        format!("{}NonNegative", base),
                        format!("{} >= 0", left),
                        format!("{} < 0", left),
                    )
                }
            } else if is_strict {
                (
                    format!("{}Negative", base),
                    format!("{} < 0", left),
                    format!("{} >= 0", left),
                )
            } else {
                (
                    format!("{}ZeroOrBelow", base),
                    format!("{} <= 0", left),
                    format!("{} > 0", left),
                )
            });
        }

        // Named constant (e.g. HIGH_VALUE_THRESHOLD)
        if right.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            let rhs_id = camelize(
                &right
                    .replace("_THRESHOLD", "")
                    .replace('_', " ")
                    .to_lowercase(),
            );
            return Ok(if is_gt {
                (
                    format!("{}{}Exceeded", base, capitalize(&rhs_id)),
                    format!("{} {} {}", left, eop, right),
                    format!("{} below {}", left, right),
                )
            } else {
                (
                    format!("{}{}BelowThreshold", base, capitalize(&rhs_id)),
                    format!("{} below {}", left, right),
                    format!("{} {} {}", left, eop, right),
                )
            });
        }

        // Numeric literal
        let rhs_id = right.replace('.', "_");
        return Ok(if is_gt {
            (
                format!("{}Above{}", base, capitalize(&rhs_id)),
                format!("{} {} {}", left, eop, right),
                format!("{} not above {}", left, right),
            )
        } else {
            (
                format!("{}Below{}", base, capitalize(&rhs_id)),
                format!("{} {} {}", left, eop, right),
                format!("{} not below {}", left, right),
            )
        });
    }

    Err(())
}

fn parse_comparison(text: &str) -> Option<(String, String, String)> {
    // Try operators in order of specificity (longer first)
    let operators = ["===", "!==", "==", "!=", ">=", "<=", ">", "<"];
    for op in &operators {
        if let Some(pos) = text.find(op) {
            let left = text[..pos].trim().to_string();
            let right = text[pos + op.len()..].trim().to_string();
            if !left.is_empty() && !right.is_empty() {
                return Some((left, op.to_string(), right));
            }
        }
    }
    None
}

fn parse_string_literal(s: &str) -> Option<String> {
    let s = s.trim();
    if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"')) {
        Some(s[1..s.len() - 1].to_string())
    } else {
        None
    }
}

fn derive_call(text: &str, negated: bool) -> (String, String, String) {
    // e.g. `canTransition(order.status, 'approved')`
    let stripped = text.trim_start_matches("await").trim();
    let stripped = stripped
        .trim_start_matches("this.")
        .trim_start_matches("await")
        .trim();
    // Find function name before first '('
    let fn_name = if let Some(paren_pos) = stripped.find('(') {
        let before = &stripped[..paren_pos];
        // Get last segment after last dot
        before.split('.').last().unwrap_or(before)
    } else {
        stripped.split('(').next().unwrap_or(stripped)
    };

    // Extract first string literal argument
    let arg_suffix = if let Some(paren_pos) = stripped.find('(') {
        let args_str = &stripped[paren_pos + 1..];
        extract_first_string_arg(args_str)
            .map(|s| capitalize(&camelize(&s)))
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Preserve original casing if already camelCase; only camelize snake_case/kebab names
    let fn_ident = if fn_name.contains('_') || fn_name.contains('-') || fn_name.contains(' ') {
        camelize(fn_name)
    } else {
        // Already camelCase — just ensure first char is lowercase
        let mut chars = fn_name.chars();
        match chars.next() {
            Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
            None => String::new(),
        }
    };
    let base = format!("{}{}", fn_ident, arg_suffix);

    if negated {
        (
            format!("{}Failed", base),
            format!("{} returned false", text),
            format!("{} returned true", text),
        )
    } else {
        (
            format!("{}Passed", base),
            format!("{} returned true", text),
            format!("{} returned false", text),
        )
    }
}

fn extract_first_string_arg(args_str: &str) -> Option<String> {
    let args_str = args_str.trim();
    // Find first string literal
    for quote in &['\'', '"'] {
        if let Some(start) = args_str.find(*quote) {
            let rest = &args_str[start + 1..];
            if let Some(end) = rest.find(*quote) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Extract the last segment of a camelCase path (e.g. "inputForceApprove" -> "forceApprove")
fn last_segment(ident: &str) -> String {
    // For compound identifiers like "riskCheckOk", find the last camelCase word
    // For simple identifiers like "order", return as-is
    ident.to_string()
}

/// Extract the last dot-segment of a path, or the full identifier
fn last_dot_segment(text: &str) -> String {
    text.split('.').last().unwrap_or(text).trim().to_string()
}

fn path_to_ident(text: &str) -> String {
    let mut s = text.trim().to_string();
    // Strip await
    if s.starts_with("await ") {
        s = s[6..].trim().to_string();
    }
    // Strip this.
    if s.starts_with("this.") {
        s = s[5..].to_string();
    }
    // Strip array indexing
    let s = strip_bracket_indexing(&s);
    // Stop at operator or paren
    let s = s
        .split(|c: char| {
            c == ' ' || c == '!' || c == '<' || c == '>' || c == '=' || c == '(' || c == ')'
        })
        .next()
        .unwrap_or(&s)
        .to_string();
    let parts: Vec<&str> = s.split('.').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return camelize(text);
    }
    let mut result = parts[0].to_string();
    for part in &parts[1..] {
        result.push_str(&capitalize(part));
    }
    result
}

fn strip_bracket_indexing(s: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '[' => depth += 1,
            ']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ => {
                if depth == 0 {
                    result.push(ch);
                }
            }
        }
    }
    result
}

fn negate_op(op: &str) -> String {
    match op {
        ">" => "<=".to_string(),
        ">=" => "<".to_string(),
        "<" => ">=".to_string(),
        "<=" => ">".to_string(),
        _ => op.to_string(),
    }
}

fn strip_leading_not(text: &str) -> String {
    let s = text.trim_start_matches('!');
    // Strip surrounding parens
    let s = s.trim();
    if s.starts_with('(') && s.ends_with(')') {
        s[1..s.len() - 1].trim().to_string()
    } else {
        s.to_string()
    }
}

fn camelize(s: &str) -> String {
    let s = s.trim();
    let parts: Vec<&str> = s
        .split(|c: char| c == ' ' || c == '_' || c == '-')
        .collect();
    parts
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if i == 0 {
                w.to_lowercase()
            } else {
                capitalize(w)
            }
        })
        .collect()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
