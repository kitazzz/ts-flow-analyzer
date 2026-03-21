import { Node, SyntaxKind } from 'ts-morph'

// Nodes that contribute to metrics — highlighted in the tree
const CC_NODES = new Set([
  SyntaxKind.IfStatement,
  SyntaxKind.CaseClause,
  SyntaxKind.ConditionalExpression,
])

const NESTING_NODES = new Set([
  SyntaxKind.SwitchStatement,
  SyntaxKind.ForStatement,
  SyntaxKind.ForInStatement,
  SyntaxKind.ForOfStatement,
  SyntaxKind.WhileStatement,
  SyntaxKind.DoStatement,
  SyntaxKind.TryStatement,
])

const FUNCTION_NODES = new Set([
  SyntaxKind.FunctionDeclaration,
  SyntaxKind.FunctionExpression,
  SyntaxKind.ArrowFunction,
  SyntaxKind.MethodDeclaration,
])

// Leaf-like nodes: show text instead of recursing into children
const LEAF_KINDS = new Set([
  SyntaxKind.Identifier,
  SyntaxKind.StringLiteral,
  SyntaxKind.NumericLiteral,
  SyntaxKind.TrueKeyword,
  SyntaxKind.FalseKeyword,
  SyntaxKind.NullKeyword,
  SyntaxKind.NoSubstitutionTemplateLiteral,
])

// Pure punctuation / noise tokens to skip entirely
const SKIP_KINDS = new Set([
  SyntaxKind.SemicolonToken,
  SyntaxKind.CommaToken,
  SyntaxKind.OpenBraceToken,
  SyntaxKind.CloseBraceToken,
  SyntaxKind.OpenParenToken,
  SyntaxKind.CloseParenToken,
  SyntaxKind.OpenBracketToken,
  SyntaxKind.CloseBracketToken,
  SyntaxKind.ColonToken,
  SyntaxKind.DotToken,
  SyntaxKind.QuestionDotToken,
  SyntaxKind.EndOfFileToken,
  SyntaxKind.ExportKeyword,
  SyntaxKind.AsyncKeyword,
  SyntaxKind.AwaitKeyword,
  SyntaxKind.ConstKeyword,
  SyntaxKind.LetKeyword,
  SyntaxKind.VarKeyword,
  SyntaxKind.FunctionKeyword,
  SyntaxKind.ReturnKeyword,
  SyntaxKind.ThrowKeyword,
  SyntaxKind.NewKeyword,
  SyntaxKind.TypeKeyword,
  SyntaxKind.EqualsToken,
  SyntaxKind.EqualsGreaterThanToken,
  SyntaxKind.QuestionToken,
  SyntaxKind.ExclamationToken,
])

function marker(kind: SyntaxKind): string {
  if (CC_NODES.has(kind)) return ' ◆[CC+1]'
  if (NESTING_NODES.has(kind)) return ' ▸[nest]'
  if (FUNCTION_NODES.has(kind)) return ' ƒ[fn]'
  return ''
}

export function printAst(node: Node, maxDepth = 8): string {
  const lines: string[] = []
  walk(node, 0, maxDepth, lines)
  return lines.join('\n')
}

function walk(node: Node, depth: number, maxDepth: number, lines: string[]): void {
  if (depth > maxDepth) {
    lines.push(`${'  '.repeat(depth)}…`)
    return
  }

  const kind = node.getKind()
  const indent = '  '.repeat(depth)
  const tag = marker(kind)

  if (SKIP_KINDS.has(kind)) return

  if (LEAF_KINDS.has(kind)) {
    lines.push(`${indent}${node.getKindName()}: "${node.getText()}"${tag}`)
    return
  }

  lines.push(`${indent}${node.getKindName()}${tag}`)

  for (const child of semanticChildren(node)) {
    walk(child, depth + 1, maxDepth, lines)
  }
}

// Flatten SyntaxList one level so it doesn't appear as a node in the tree
function semanticChildren(node: Node): Node[] {
  const result: Node[] = []
  for (const child of node.getChildren()) {
    if (child.getKind() === SyntaxKind.SyntaxList) {
      result.push(...child.getChildren())
    } else {
      result.push(child)
    }
  }
  return result
}
