import { Node, SyntaxKind } from 'ts-morph'

const LOCAL_FUNCTION_KINDS = new Set([
  SyntaxKind.FunctionDeclaration,
  SyntaxKind.FunctionExpression,
  SyntaxKind.ArrowFunction,
  SyntaxKind.MethodDeclaration,
])

const CALLBACK_FUNCTION_KINDS = new Set([
  SyntaxKind.FunctionExpression,
  SyntaxKind.ArrowFunction,
])

const CALL_LIKE_KINDS = new Set([
  SyntaxKind.CallExpression,
  SyntaxKind.NewExpression,
])

type FunctionNesting = {
  localFunctionCount: number
  functionNestingDepth: number
  callbackNestingDepth: number
}

export function analyzeFunctionNesting(fnNode: Node): FunctionNesting {
  return {
    localFunctionCount: countLocalFunctions(fnNode),
    functionNestingDepth: calcFunctionNestingDepth(fnNode),
    callbackNestingDepth: calcCallbackNestingDepth(fnNode, 0),
  }
}

// Body 内の全関数の総数（自身を除く、深さ問わず）
function countLocalFunctions(fnNode: Node): number {
  let count = 0
  for (const node of fnNode.getDescendants()) {
    if (LOCAL_FUNCTION_KINDS.has(node.getKind())) {
      count++
    }
  }
  return count
}

// 対象関数を深さ0として、内部の関数ネストの最大深度
function calcFunctionNestingDepth(fnNode: Node): number {
  return walkFunctionDepth(fnNode, 0, true)
}

function walkFunctionDepth(node: Node, depth: number, isRoot: boolean): number {
  let max = depth

  for (const child of node.getChildren()) {
    const isNestedFn = LOCAL_FUNCTION_KINDS.has(child.getKind())
    const childDepth = isNestedFn && !isRoot ? depth + 1 : depth
    const result = walkFunctionDepth(child, childDepth, false)
    if (result > max) max = result
  }

  return max
}

// CallExpression.arguments に現れる ArrowFunction | FunctionExpression のネスト最大深度
function calcCallbackNestingDepth(node: Node, depth: number): number {
  let max = depth

  for (const child of node.getChildren()) {
    const kind = child.getKind()

    if (CALL_LIKE_KINDS.has(kind)) {
      // CallExpression or NewExpression
      const args: Node[] = kind === SyntaxKind.CallExpression
        ? child.asKindOrThrow(SyntaxKind.CallExpression).getArguments()
        : child.asKindOrThrow(SyntaxKind.NewExpression).getArguments()

      for (const arg of args) {
        const argKind = arg.getKind()
        if (CALLBACK_FUNCTION_KINDS.has(argKind)) {
          // この引数はコールバック → 深さ+1 で再帰
          const inner = calcCallbackNestingDepth(arg, depth + 1)
          if (inner > max) max = inner
        } else {
          const inner = calcCallbackNestingDepth(arg, depth)
          if (inner > max) max = inner
        }
      }

      // CallExpression の関数部分も走査（.then(cb) の .then 側など）
      if (kind === SyntaxKind.CallExpression) {
        const inner = calcCallbackNestingDepth(
          child.asKindOrThrow(SyntaxKind.CallExpression).getExpression(),
          depth,
        )
        if (inner > max) max = inner
      }
    } else if (kind !== SyntaxKind.FunctionDeclaration && kind !== SyntaxKind.FunctionExpression && kind !== SyntaxKind.ArrowFunction) {
      // 関数ノード自体はここでは再帰しない（CallExpression の引数として処理済み）
      const inner = calcCallbackNestingDepth(child, depth)
      if (inner > max) max = inner
    }
  }

  return max
}
