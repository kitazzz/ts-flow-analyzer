use oxc_ast::ast::*;
use oxc_span::GetSpan;
use crate::model::PredicateKind;

pub fn classify_predicate(expr: &Expression<'_>, source: &str) -> PredicateKind {
    match expr {
        Expression::BinaryExpression(bin) => {
            let right = &bin.right;
            let right_text = &source[right.span().start as usize..right.span().end as usize];

            // null / undefined checks
            let is_null_check = matches!(
                bin.operator,
                BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
            ) && (matches!(right, Expression::NullLiteral(_))
                || right_text == "undefined"
                || matches!(right, Expression::Identifier(id) if id.name == "undefined"));

            if is_null_check {
                return PredicateKind::NullCheck;
            }

            // instanceof
            if matches!(bin.operator, BinaryOperator::Instanceof) {
                return PredicateKind::TypeCheck;
            }

            // Check if left is typeof
            if let Expression::UnaryExpression(u) = &bin.left {
                if matches!(u.operator, UnaryOperator::Typeof) {
                    return PredicateKind::TypeCheck;
                }
            }

            // Comparison operators
            if matches!(
                bin.operator,
                BinaryOperator::LessThan
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterEqualThan
                    | BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
                    | BinaryOperator::Equality
                    | BinaryOperator::Inequality
            ) {
                return PredicateKind::Comparison;
            }

            PredicateKind::Other
        }

        Expression::UnaryExpression(u) => {
            if matches!(u.operator, UnaryOperator::LogicalNot) {
                PredicateKind::Negation
            } else if matches!(u.operator, UnaryOperator::Typeof) {
                PredicateKind::TypeCheck
            } else {
                PredicateKind::Other
            }
        }

        Expression::CallExpression(_) | Expression::AwaitExpression(_) => PredicateKind::Call,

        Expression::Identifier(_)
        | Expression::StaticMemberExpression(_)
        | Expression::ComputedMemberExpression(_)
        | Expression::TSNonNullExpression(_) => PredicateKind::Truthiness,

        _ => PredicateKind::Other,
    }
}
