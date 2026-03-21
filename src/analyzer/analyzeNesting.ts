import { Node, SyntaxKind } from 'ts-morph'

const NESTING_KINDS = new Set([
  SyntaxKind.IfStatement,
  SyntaxKind.SwitchStatement,
  SyntaxKind.ForStatement,
  SyntaxKind.ForInStatement,
  SyntaxKind.ForOfStatement,
  SyntaxKind.WhileStatement,
  SyntaxKind.DoStatement,
  SyntaxKind.TryStatement,
])

// Function nesting is tracked separately — skip nested functions to avoid double-counting
const FUNCTION_KINDS = new Set([
  SyntaxKind.FunctionDeclaration,
  SyntaxKind.FunctionExpression,
  SyntaxKind.ArrowFunction,
])

export function analyzeMaxNestingDepth(fnNode: Node): number {
  return walkNesting(fnNode, 0, true)
}

function walkNesting(node: Node, currentDepth: number, isRoot: boolean): number {
  let max = currentDepth

  for (const child of node.getChildren()) {
    // Skip nested functions (they are counted separately)
    if (!isRoot && FUNCTION_KINDS.has(child.getKind())) continue

    const childDepth = NESTING_KINDS.has(child.getKind()) ? currentDepth + 1 : currentDepth
    const result = walkNesting(child, childDepth, false)
    if (result > max) max = result
  }

  return max
}
