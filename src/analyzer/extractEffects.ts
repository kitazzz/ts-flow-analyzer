import { Node, SyntaxKind } from 'ts-morph'
import type { Effect, EffectKind, SideEffectClass } from '../model/Effect.ts'

// Keywords that suggest DB write operations (camelCase prefix match)
const DB_WRITE_PATTERN = /\b(save|update|delete|remove|insert|upsert|create|put|patch)(?=[A-Z(]|\b)/i
// Keywords that suggest DB read operations (camelCase prefix match)
const DB_READ_PATTERN = /\b(find|get|query|fetch|load|select|search|list)(?=[A-Z(]|\b)/i
// Keywords that suggest external API / messaging
const EXTERNAL_PATTERN = /\b(http|axios|request|post|send|publish|emit|notify|dispatch)(?=[A-Z(]|\b)/i
// Keywords that suggest logging
const LOG_PATTERN = /\b(log|warn|debug|console|logger|trace)(?=[A-Z(.]|\b)/i

export function extractEffects(fnNode: Node): Effect[] {
  const results: Effect[] = []

  for (const node of fnNode.getDescendants()) {
    const kind = node.getKind()

    if (kind === SyntaxKind.ReturnStatement) {
      results.push({
        kind: 'return',
        sideEffect: 'return',
        text: node.getText().trim(),
        line: node.getStartLineNumber(),
      })
      continue
    }

    if (kind === SyntaxKind.ThrowStatement) {
      results.push({
        kind: 'throw',
        sideEffect: 'throw',
        text: node.getText().trim(),
        line: node.getStartLineNumber(),
      })
      continue
    }

    // Expression statements (calls, assignments at statement level)
    if (kind === SyntaxKind.ExpressionStatement) {
      const expr = node.asKindOrThrow(SyntaxKind.ExpressionStatement).getExpression()
      const exprKind = expr.getKind()

      if (
        exprKind === SyntaxKind.BinaryExpression &&
        expr.asKindOrThrow(SyntaxKind.BinaryExpression).getOperatorToken().getKind() === SyntaxKind.EqualsToken
      ) {
        // Assignment
        const text = node.getText().trim()
        results.push({
          kind: 'assignment',
          sideEffect: 'stateWrite',
          text,
          line: node.getStartLineNumber(),
        })
        continue
      }

      if (exprKind === SyntaxKind.CallExpression || exprKind === SyntaxKind.AwaitExpression) {
        const text = node.getText().trim()
        results.push({
          kind: 'call',
          sideEffect: classifyCallSideEffect(text),
          text,
          line: node.getStartLineNumber(),
        })
        continue
      }
    }

    // Variable declarations with initializer that is a call (e.g. const x = await repo.find())
    if (kind === SyntaxKind.VariableStatement) {
      const text = node.getText().trim()
      const hasCall =
        text.includes('(') &&
        (text.includes('await ') || text.includes('= '))
      if (hasCall) {
        results.push({
          kind: 'call',
          sideEffect: classifyCallSideEffect(text),
          text,
          line: node.getStartLineNumber(),
        })
      }
    }
  }

  return results
}

function classifyCallSideEffect(text: string): SideEffectClass {
  if (LOG_PATTERN.test(text)) return 'logging'
  if (DB_WRITE_PATTERN.test(text)) return 'dbWrite'
  if (DB_READ_PATTERN.test(text)) return 'dbRead'
  if (EXTERNAL_PATTERN.test(text)) return 'externalApi'
  return 'pureCall'
}
