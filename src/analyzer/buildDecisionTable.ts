import {
  Node,
  SyntaxKind,
  type ArrowFunction,
  type FunctionDeclaration,
  type FunctionExpression,
  type IfStatement,
  type MethodDeclaration,
  type Statement,
} from 'ts-morph'
import type { FunctionReport } from '../model/FunctionReport.ts'
import type { DecisionPoint, DecisionTable, McdcCase, TruthRow } from '../model/DecisionTable.ts'

type ReportWithNode = FunctionReport & { node?: Node }
type DecisionValue = 'T' | 'F' | '*'
type PathState = Map<number, Exclude<DecisionValue, '*'>>
type PathOutcome = {
  assignments: PathState
  outcome: string
  outcomeKind: 'return' | 'throw'
  line: number
}
type WalkResult = {
  terminated: PathOutcome[]
  continued: PathState[]
}

export function buildDecisionTable(report: ReportWithNode): DecisionTable | null {
  if (!report.node || report.symbolKind === 'class') return null

  const statements = getRootStatements(report.node)
  if (statements.length === 0) return null

  const decisions: DecisionPoint[] = []
  const walk = walkStatements(statements, [new Map()], decisions)
  if (walk.terminated.length === 0 || decisions.length === 0) return null

  const successLine = findSuccessLine(walk.terminated)
  const truthRows = dedupeRows(
    walk.terminated.map((path) => toTruthRow(path, decisions.length, successLine)),
  )
  if (truthRows.length === 0) return null

  const happyRows = truthRows.filter((row) => row.outcomeKind === 'happy')
  const happyPath = happyRows
    .slice()
    .sort((a, b) => concreteCount(b.values) - concreteCount(a.values))[0] ?? null

  return {
    symbolName: report.symbolName,
    symbolKind: report.symbolKind,
    decisions,
    truthRows,
    mcdcCases: buildMcdcCases(decisions, truthRows),
    happyPath,
  }
}

function getRootStatements(
  node: Node,
): Statement[] {
  const kind = node.getKind()

  if (
    kind === SyntaxKind.FunctionDeclaration ||
    kind === SyntaxKind.FunctionExpression ||
    kind === SyntaxKind.MethodDeclaration
  ) {
    const body = (
      node as FunctionDeclaration | FunctionExpression | MethodDeclaration
    ).getBody()
    return body && body.getKind() === SyntaxKind.Block
      ? body.asKindOrThrow(SyntaxKind.Block).getStatements()
      : []
  }

  if (kind === SyntaxKind.ArrowFunction) {
    const body = (node as ArrowFunction).getBody()
    return body.getKind() === SyntaxKind.Block
      ? body.asKindOrThrow(SyntaxKind.Block).getStatements()
      : []
  }

  return []
}

function walkStatements(
  statements: Statement[],
  currentPaths: PathState[],
  decisions: DecisionPoint[],
): WalkResult {
  let continued = currentPaths
  const terminated: PathOutcome[] = []

  for (const statement of statements) {
    if (continued.length === 0) break

    const next: PathState[] = []
    for (const path of continued) {
      const result = walkStatement(statement, path, decisions)
      terminated.push(...result.terminated)
      next.push(...result.continued)
    }
    continued = next
  }

  return { terminated, continued }
}

function walkStatement(
  statement: Statement,
  path: PathState,
  decisions: DecisionPoint[],
): WalkResult {
  const kind = statement.getKind()

  if (kind === SyntaxKind.ReturnStatement) {
    return {
      terminated: [{
        assignments: path,
        outcome: compact(statement.getText()),
        outcomeKind: 'return',
        line: statement.getStartLineNumber(),
      }],
      continued: [],
    }
  }

  if (kind === SyntaxKind.ThrowStatement) {
    return {
      terminated: [{
        assignments: path,
        outcome: compact(statement.getText()),
        outcomeKind: 'throw',
        line: statement.getStartLineNumber(),
      }],
      continued: [],
    }
  }

  if (kind === SyntaxKind.Block) {
    return walkStatements(
      statement.asKindOrThrow(SyntaxKind.Block).getStatements(),
      [path],
      decisions,
    )
  }

  if (kind === SyntaxKind.IfStatement) {
    return walkIfStatement(statement.asKindOrThrow(SyntaxKind.IfStatement), path, decisions)
  }

  return { terminated: [], continued: [path] }
}

function walkIfStatement(
  ifNode: IfStatement,
  path: PathState,
  decisions: DecisionPoint[],
): WalkResult {
  const index = decisions.length
  decisions.push({
    index,
    predicate: compact(ifNode.getExpression().getText()),
    line: ifNode.getStartLineNumber(),
  })

  const thenPath = new Map(path)
  thenPath.set(index, 'T')
  const thenResult = walkBranch(ifNode.getThenStatement(), thenPath, decisions)

  const elsePath = new Map(path)
  elsePath.set(index, 'F')
  const elseStmt = ifNode.getElseStatement()
  const elseResult = elseStmt
    ? walkBranch(elseStmt, elsePath, decisions)
    : { terminated: [], continued: [elsePath] }

  return {
    terminated: [...thenResult.terminated, ...elseResult.terminated],
    continued: [...thenResult.continued, ...elseResult.continued],
  }
}

function walkBranch(
  statement: Statement,
  path: PathState,
  decisions: DecisionPoint[],
): WalkResult {
  if (statement.getKind() === SyntaxKind.Block) {
    const block = statement.asKindOrThrow(SyntaxKind.Block)
    return walkStatements(block.getStatements(), [path], decisions)
  }
  return walkStatement(statement, path, decisions)
}

function findSuccessLine(outcomes: PathOutcome[]): number | null {
  const returnLines = outcomes
    .filter((outcome) => outcome.outcomeKind === 'return')
    .map((outcome) => outcome.line)

  if (returnLines.length === 0) return null
  return Math.max(...returnLines)
}

function toTruthRow(path: PathOutcome, size: number, successLine: number | null): TruthRow {
  const values: DecisionValue[] = Array(size).fill('*')
  for (const [index, value] of path.assignments) {
    values[index] = value
  }

  const isHappy = successLine !== null && path.outcomeKind === 'return' && path.line === successLine

  return {
    values,
    outcome: path.outcome,
    outcomeLabel: summarizeOutcome(path.outcome, isHappy),
    outcomeKind: isHappy ? 'happy' : path.outcomeKind,
    line: path.line,
  }
}

function dedupeRows(rows: TruthRow[]): TruthRow[] {
  const seen = new Set<string>()
  const result: TruthRow[] = []

  for (const row of rows) {
    const key = `${row.values.join('')}:${row.line}:${row.outcome}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(row)
  }

  return result
}

function buildMcdcCases(decisions: DecisionPoint[], rows: TruthRow[]): McdcCase[] {
  const cases: McdcCase[] = []

  for (const decision of decisions) {
    const pair = findMcdcPair(rows, decision.index)
    if (!pair) continue

    cases.push({
      predicate: decision.predicate,
      index: decision.index,
      rowFalse: pair.rowFalse,
      rowTrue: pair.rowTrue,
    })
  }

  return cases
}

function findMcdcPair(
  rows: TruthRow[],
  index: number,
): { rowFalse: TruthRow; rowTrue: TruthRow } | null {
  let best: { rowFalse: TruthRow; rowTrue: TruthRow; score: number } | null = null

  for (const rowFalse of rows) {
    if (rowFalse.values[index] !== 'F') continue

    for (const rowTrue of rows) {
      if (rowTrue.values[index] !== 'T') continue
      if (rowFalse.outcomeLabel === rowTrue.outcomeLabel) continue

      const score = compareRowsForMcdc(rowFalse, rowTrue, index)
      if (score < 0) continue

      if (!best || score > best.score) {
        best = { rowFalse, rowTrue, score }
      }
    }
  }

  return best ? { rowFalse: best.rowFalse, rowTrue: best.rowTrue } : null
}

function compareRowsForMcdc(a: TruthRow, b: TruthRow, focus: number): number {
  let score = 0

  for (let i = 0; i < a.values.length; i++) {
    if (i === focus) continue

    const left = a.values[i]
    const right = b.values[i]
    if (left === right) {
      if (left !== '*') score += 2
      continue
    }
    if (left === '*' || right === '*') {
      score += 1
      continue
    }
    return -1
  }

  if (a.outcomeKind === 'happy' || b.outcomeKind === 'happy') score += 10
  if (a.outcomeKind === 'happy') score += 2
  if (b.outcomeKind === 'happy') score += 2

  return score
}

function summarizeOutcome(text: string, isHappy: boolean): string {
  if (isHappy) return 'Success'

  const candidates = [
    /(?:error|reason)\s*:\s*'([^']+)'/i,
    /(?:error|reason)\s*:\s*"([^"]+)"/i,
    /(?:error|reason)\s*:\s*`([^`$]+)(?:\$\{.*)?`/i,
  ]

  for (const pattern of candidates) {
    const match = text.match(pattern)
    if (!match) continue

    const label = toPascalCase(match[1])
    if (label) return label
  }

  if (text.startsWith('throw ')) return 'Throw'
  if (text.includes('ok: false')) return 'Failure'
  if (text.includes('ok: true')) return 'Success'
  return 'Outcome'
}

function toPascalCase(text: string): string {
  const parts = text
    .replace(/[¥$]/g, ' ')
    .replace(/[^a-zA-Z0-9]+/g, ' ')
    .trim()
    .split(/\s+/)
    .filter(Boolean)

  return parts
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('')
}

function compact(text: string): string {
  return text.replace(/\s+/g, ' ').trim()
}

function concreteCount(values: DecisionValue[]): number {
  return values.filter((value) => value !== '*').length
}
