import {
  SourceFile,
  SyntaxKind,
  FunctionDeclaration,
  ArrowFunction,
  FunctionExpression,
  MethodDeclaration,
  ClassDeclaration,
  Node,
} from 'ts-morph'
import type { SymbolKind } from '../model/FunctionReport.ts'

export type AnalyzableNode =
  | FunctionDeclaration
  | ArrowFunction
  | FunctionExpression
  | MethodDeclaration
  | ClassDeclaration

export type CollectedFunction = {
  symbolName: string
  symbolKind: SymbolKind
  className?: string
  memberName?: string
  node: AnalyzableNode
  filePath: string
  startLine: number
}

export function collectFunctions(sourceFile: SourceFile): CollectedFunction[] {
  const results: CollectedFunction[] = []
  const filePath = sourceFile.getFilePath()

  // FunctionDeclaration: function foo() {}
  for (const fn of sourceFile.getDescendantsOfKind(SyntaxKind.FunctionDeclaration)) {
    const name = fn.getName()
    if (name) {
      results.push({
        symbolName: name,
        symbolKind: 'function',
        memberName: name,
        node: fn,
        filePath,
        startLine: fn.getStartLineNumber(),
      })
    }
  }

  // VariableDeclaration の initializer が ArrowFunction | FunctionExpression
  for (const varDecl of sourceFile.getDescendantsOfKind(SyntaxKind.VariableDeclaration)) {
    const initializer = varDecl.getInitializer()
    if (!initializer) continue

    const kind = initializer.getKind()
    if (kind !== SyntaxKind.ArrowFunction && kind !== SyntaxKind.FunctionExpression) continue

    const name = varDecl.getName()
    if (!name) continue

    results.push({
      symbolName: name,
      symbolKind: 'variableFunction',
      memberName: name,
      node: initializer as ArrowFunction | FunctionExpression,
      filePath,
      startLine: varDecl.getStartLineNumber(),
    })
  }

  // MethodDeclaration: class methods (including private)
  for (const method of sourceFile.getDescendantsOfKind(SyntaxKind.MethodDeclaration)) {
    const name = method.getName()
    if (!name) continue

    // クラス名を prefix に付ける: ClassName#methodName
    const parent = method.getParent()
    const className =
      parent && parent.getKind() === SyntaxKind.ClassDeclaration
        ? (parent.asKindOrThrow(SyntaxKind.ClassDeclaration).getName() ?? '')
        : ''

    results.push({
      symbolName: className ? `${className}#${name}` : name,
      symbolKind: 'method',
      className: className || undefined,
      memberName: name,
      node: method,
      filePath,
      startLine: method.getStartLineNumber(),
    })
  }

  // ClassDeclaration: named classes
  for (const classDecl of sourceFile.getDescendantsOfKind(SyntaxKind.ClassDeclaration)) {
    const name = classDecl.getName()
    if (!name) continue

    results.push({
      symbolName: name,
      symbolKind: 'class',
      className: name,
      node: classDecl,
      filePath,
      startLine: classDecl.getStartLineNumber(),
    })
  }

  return results
}

export function isInsideFunction(node: Node): boolean {
  let current = node.getParent()
  while (current) {
    const kind = current.getKind()
    if (
      kind === SyntaxKind.FunctionDeclaration ||
      kind === SyntaxKind.FunctionExpression ||
      kind === SyntaxKind.ArrowFunction ||
      kind === SyntaxKind.MethodDeclaration
    ) {
      return true
    }
    current = current.getParent()
  }
  return false
}
