import { Node, SyntaxKind } from 'ts-morph'

type BasicComplexity = {
  ifCount: number
  elseIfCount: number
  switchCount: number
  ternaryCount: number
  returnCount: number
}

export function analyzeBasicComplexity(fnNode: Node): BasicComplexity {
  let ifCount = 0
  let elseIfCount = 0
  let switchCount = 0
  let ternaryCount = 0
  let returnCount = 0

  for (const node of fnNode.getDescendants()) {
    const kind = node.getKind()

    if (kind === SyntaxKind.IfStatement) {
      ifCount++
      // else if: 親が IfStatement で、自分が elseStatement に当たるもの
      const parent = node.getParent()
      if (parent && parent.getKind() === SyntaxKind.IfStatement) {
        // ts-morph: IfStatement の getElseStatement() で取得
        const parentIf = parent.asKindOrThrow(SyntaxKind.IfStatement)
        if (parentIf.getElseStatement() === node) {
          elseIfCount++
        }
      }
    } else if (kind === SyntaxKind.SwitchStatement) {
      switchCount++
    } else if (kind === SyntaxKind.ConditionalExpression) {
      ternaryCount++
    } else if (kind === SyntaxKind.ReturnStatement) {
      returnCount++
    }
  }

  return { ifCount, elseIfCount, switchCount, ternaryCount, returnCount }
}
