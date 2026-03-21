import { Node, SyntaxKind } from 'ts-morph'

// Condition nodes that introduce branching decisions
const DECISION_NODES = new Set([
  SyntaxKind.IfStatement,
  SyntaxKind.ConditionalExpression,
  SyntaxKind.CaseClause,
  SyntaxKind.WhileStatement,
  SyntaxKind.DoStatement,
  SyntaxKind.ForStatement,
])

export type ConditionMetrics = {
  negationCount: number
  atomicConditionCount: number
  maxConditionDepth: number
}

export function analyzeConditionComplexity(fnNode: Node): ConditionMetrics {
  let negationCount = 0
  let atomicConditionCount = 0
  let maxConditionDepth = 0

  for (const node of fnNode.getDescendants()) {
    const kind = node.getKind()

    if (kind === SyntaxKind.PrefixUnaryExpression) {
      // ! operator
      if (node.asKindOrThrow(SyntaxKind.PrefixUnaryExpression).getOperatorToken() === SyntaxKind.ExclamationToken) {
        negationCount++
      }
    }

    // Analyze condition expressions at decision points
    if (DECISION_NODES.has(kind)) {
      const condExpr = getConditionExpression(node)
      if (condExpr) {
        const { atomics, depth } = analyzeLogicalExpr(condExpr)
        atomicConditionCount += atomics
        if (depth > maxConditionDepth) maxConditionDepth = depth
      }
    }
  }

  return { negationCount, atomicConditionCount, maxConditionDepth }
}

function getConditionExpression(node: Node): Node | undefined {
  const kind = node.getKind()
  if (kind === SyntaxKind.IfStatement) {
    return node.asKindOrThrow(SyntaxKind.IfStatement).getExpression()
  }
  if (kind === SyntaxKind.ConditionalExpression) {
    return node.asKindOrThrow(SyntaxKind.ConditionalExpression).getCondition()
  }
  if (kind === SyntaxKind.WhileStatement) {
    return node.asKindOrThrow(SyntaxKind.WhileStatement).getExpression()
  }
  if (kind === SyntaxKind.DoStatement) {
    return node.asKindOrThrow(SyntaxKind.DoStatement).getExpression()
  }
  if (kind === SyntaxKind.ForStatement) {
    return node.asKindOrThrow(SyntaxKind.ForStatement).getCondition() ?? undefined
  }
  if (kind === SyntaxKind.CaseClause) {
    return node.asKindOrThrow(SyntaxKind.CaseClause).getExpression()
  }
  return undefined
}

// Returns atomic predicate count and logical nesting depth for an expression
function analyzeLogicalExpr(node: Node): { atomics: number; depth: number } {
  const kind = node.getKind()

  if (kind === SyntaxKind.BinaryExpression) {
    const bin = node.asKindOrThrow(SyntaxKind.BinaryExpression)
    const op = bin.getOperatorToken().getKind()

    if (op === SyntaxKind.AmpersandAmpersandToken || op === SyntaxKind.BarBarToken || op === SyntaxKind.QuestionQuestionToken) {
      const left = analyzeLogicalExpr(bin.getLeft())
      const right = analyzeLogicalExpr(bin.getRight())
      return {
        atomics: left.atomics + right.atomics,
        depth: 1 + Math.max(left.depth, right.depth),
      }
    }
  }

  if (kind === SyntaxKind.PrefixUnaryExpression) {
    const inner = analyzeLogicalExpr(
      node.asKindOrThrow(SyntaxKind.PrefixUnaryExpression).getOperand()
    )
    return { atomics: inner.atomics, depth: inner.depth }
  }

  if (kind === SyntaxKind.ParenthesizedExpression) {
    return analyzeLogicalExpr(
      node.asKindOrThrow(SyntaxKind.ParenthesizedExpression).getExpression()
    )
  }

  // Leaf: one atomic condition
  return { atomics: 1, depth: 0 }
}
