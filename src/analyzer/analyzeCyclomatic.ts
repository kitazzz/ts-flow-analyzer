import { Node, SyntaxKind } from 'ts-morph'

export function analyzeCyclomaticComplexity(fnNode: Node): number {
  let cc = 1

  for (const node of fnNode.getDescendants()) {
    const kind = node.getKind()

    if (kind === SyntaxKind.IfStatement) {
      cc++
    } else if (kind === SyntaxKind.CaseClause) {
      // DefaultClause は数えない
      cc++
    } else if (kind === SyntaxKind.ConditionalExpression) {
      cc++
    }
  }

  return cc
}
