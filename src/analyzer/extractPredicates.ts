import { Node, SyntaxKind } from 'ts-morph'
import type { AtomicPredicate, DecisionContext, PredicateKind } from '../model/Predicate.ts'

export function extractPredicates(fnNode: Node): AtomicPredicate[] {
  const results: AtomicPredicate[] = []

  for (const node of fnNode.getDescendants()) {
    const kind = node.getKind()

    if (kind === SyntaxKind.IfStatement) {
      const ifNode = node.asKindOrThrow(SyntaxKind.IfStatement)
      const parent = node.getParent()
      const isElseIf =
        parent?.getKind() === SyntaxKind.IfStatement &&
        parent.asKindOrThrow(SyntaxKind.IfStatement).getElseStatement() === node
      const ctx: DecisionContext = isElseIf ? 'elseif' : 'if'
      collectFromExpr(ifNode.getExpression(), ctx, false, results)
      continue
    }

    if (kind === SyntaxKind.ConditionalExpression) {
      collectFromExpr(
        node.asKindOrThrow(SyntaxKind.ConditionalExpression).getCondition(),
        'ternary',
        false,
        results,
      )
      continue
    }

    if (kind === SyntaxKind.WhileStatement) {
      collectFromExpr(
        node.asKindOrThrow(SyntaxKind.WhileStatement).getExpression(),
        'while',
        false,
        results,
      )
      continue
    }

    if (kind === SyntaxKind.DoStatement) {
      collectFromExpr(
        node.asKindOrThrow(SyntaxKind.DoStatement).getExpression(),
        'doWhile',
        false,
        results,
      )
      continue
    }

    if (kind === SyntaxKind.ForStatement) {
      const cond = node.asKindOrThrow(SyntaxKind.ForStatement).getCondition()
      if (cond) collectFromExpr(cond, 'for', false, results)
      continue
    }

    if (kind === SyntaxKind.CaseClause) {
      collectFromExpr(
        node.asKindOrThrow(SyntaxKind.CaseClause).getExpression(),
        'case',
        false,
        results,
      )
      continue
    }
  }

  return results
}

function collectFromExpr(
  node: Node,
  ctx: DecisionContext,
  negated: boolean,
  out: AtomicPredicate[],
): void {
  const kind = node.getKind()

  // Logical AND / OR / nullish coalescing: recurse into both sides
  if (kind === SyntaxKind.BinaryExpression) {
    const bin = node.asKindOrThrow(SyntaxKind.BinaryExpression)
    const op = bin.getOperatorToken().getKind()
    if (
      op === SyntaxKind.AmpersandAmpersandToken ||
      op === SyntaxKind.BarBarToken ||
      op === SyntaxKind.QuestionQuestionToken
    ) {
      collectFromExpr(bin.getLeft(), ctx, negated, out)
      collectFromExpr(bin.getRight(), ctx, negated, out)
      return
    }
  }

  // Parenthesized: unwrap
  if (kind === SyntaxKind.ParenthesizedExpression) {
    collectFromExpr(
      node.asKindOrThrow(SyntaxKind.ParenthesizedExpression).getExpression(),
      ctx,
      negated,
      out,
    )
    return
  }

  // Negation: flip and recurse
  if (kind === SyntaxKind.PrefixUnaryExpression) {
    const prefix = node.asKindOrThrow(SyntaxKind.PrefixUnaryExpression)
    if (prefix.getOperatorToken() === SyntaxKind.ExclamationToken) {
      collectFromExpr(prefix.getOperand(), ctx, !negated, out)
      return
    }
  }

  // Leaf: classify and record
  out.push({
    text: node.getText().trim(),
    negated,
    kind: classifyPredicate(node),
    context: ctx,
    line: node.getStartLineNumber(),
  })
}

function classifyPredicate(node: Node): PredicateKind {
  const kind = node.getKind()

  if (kind === SyntaxKind.BinaryExpression) {
    const bin = node.asKindOrThrow(SyntaxKind.BinaryExpression)
    const op = bin.getOperatorToken().getKind()
    const right = bin.getRight()
    const rightKind = right.getKind()

    // null / undefined checks
    if (
      (op === SyntaxKind.EqualsEqualsToken || op === SyntaxKind.ExclamationEqualsToken ||
       op === SyntaxKind.EqualsEqualsEqualsToken || op === SyntaxKind.ExclamationEqualsEqualsToken) &&
      (rightKind === SyntaxKind.NullKeyword || rightKind === SyntaxKind.UndefinedKeyword ||
       right.getText() === 'undefined')
    ) {
      return 'nullCheck'
    }

    // instanceof
    if (op === SyntaxKind.InstanceOfKeyword) return 'typeCheck'

    // comparison operators
    if (
      op === SyntaxKind.GreaterThanToken ||
      op === SyntaxKind.GreaterThanEqualsToken ||
      op === SyntaxKind.LessThanToken ||
      op === SyntaxKind.LessThanEqualsToken ||
      op === SyntaxKind.EqualsEqualsEqualsToken ||
      op === SyntaxKind.ExclamationEqualsEqualsToken ||
      op === SyntaxKind.EqualsEqualsToken ||
      op === SyntaxKind.ExclamationEqualsToken
    ) {
      return 'comparison'
    }
  }

  // typeof x === '...'
  if (kind === SyntaxKind.PrefixUnaryExpression) {
    return 'negation'
  }

  if (
    kind === SyntaxKind.TypeOfExpression ||
    (kind === SyntaxKind.BinaryExpression &&
      node.asKindOrThrow(SyntaxKind.BinaryExpression).getLeft().getKind() === SyntaxKind.TypeOfExpression)
  ) {
    return 'typeCheck'
  }

  // Call expressions used as conditions
  if (kind === SyntaxKind.CallExpression || kind === SyntaxKind.AwaitExpression) {
    return 'call'
  }

  // Plain identifiers / property access — truthiness check
  if (
    kind === SyntaxKind.Identifier ||
    kind === SyntaxKind.PropertyAccessExpression ||
    kind === SyntaxKind.ElementAccessExpression ||
    kind === SyntaxKind.NonNullExpression
  ) {
    return 'truthiness'
  }

  return 'other'
}
