import type { FunctionReport } from '../model/FunctionReport.ts'
import type { AtomicPredicate } from '../model/Predicate.ts'
import type { Effect } from '../model/Effect.ts'
import type { DecisionPoint, DecisionTable, McdcCase, TruthRow } from '../model/DecisionTable.ts'

// Maximum number of decision points to include in the table (2^N rows)
const MAX_DECISIONS = 8

export function buildDecisionTable(report: FunctionReport): DecisionTable | null {
  const predicates = report.predicates ?? []
  const effects = report.effects ?? []

  if (predicates.length === 0 || effects.length === 0) return null

  // Only consider if/elseif predicates as decision points
  const ifPredicates = predicates.filter(p => p.context === 'if' || p.context === 'elseif')
  if (ifPredicates.length === 0) return null

  // Pair each if-predicate with the closest return/throw that follows it
  const decisions = pairPredicatesWithEffects(ifPredicates, effects)
  if (decisions.length === 0) return null

  const capped = decisions.slice(0, MAX_DECISIONS)

  // Find happy path: the return effect after all decisions (highest line among unpaired returns)
  const usedLines = new Set(capped.map(d => d.line))
  const happyReturn = effects
    .filter(e => (e.kind === 'return') && !usedLines.has(e.line))
    .sort((a, b) => b.line - a.line)[0]

  const happyOutcome = happyReturn?.text ?? '/* no explicit return */'

  // Build truth table (linear guard chain model)
  const truthRows = buildTruthRows(capped, happyOutcome)

  // Build MC/DC cases
  const happyRow = truthRows[truthRows.length - 1]
  const mcdcCases = buildMcdcCases(capped, truthRows, happyRow)

  return {
    symbolName: report.symbolName,
    symbolKind: report.symbolKind,
    decisions: capped,
    truthRows,
    mcdcCases,
    happyPath: happyRow,
  }
}

// Pair each if-predicate with the nearest subsequent return/throw
function pairPredicatesWithEffects(
  preds: AtomicPredicate[],
  effects: Effect[],
): DecisionPoint[] {
  const earlyExits = effects.filter(e => e.kind === 'return' || e.kind === 'throw')
  const usedEffectLines = new Set<number>()

  const result: DecisionPoint[] = []

  for (let i = 0; i < preds.length; i++) {
    const pred = preds[i]
    // Find the nearest effect after this predicate that hasn't been used yet
    const match = earlyExits
      .filter(e => e.line >= pred.line && !usedEffectLines.has(e.line))
      .sort((a, b) => a.line - b.line)[0]

    if (!match) continue

    usedEffectLines.add(match.line)
    result.push({
      index: i,
      predicate: pred.text,
      negated: pred.negated,
      predicateKind: pred.kind,
      outcome: match.text,
      outcomeKind: match.kind as 'return' | 'throw',
      line: pred.line,
    })
  }

  return result
}

// Build linear guard chain truth table:
// Row i: decisions[0..i-1] pass, decisions[i] fails → outcome[i]
// Last row: all pass → happy path
function buildTruthRows(decisions: DecisionPoint[], happyOutcome: string): TruthRow[] {
  const n = decisions.length
  const rows: TruthRow[] = []

  for (let i = 0; i < n; i++) {
    const values: ('T' | 'F' | '*')[] = decisions.map((d, j) => {
      if (j < i) return 'T'   // earlier guards passed
      if (j === i) return 'F' // this guard fires (predicate is false → negated guard triggers)
      return '*'               // don't care
    })
    rows.push({
      values,
      outcome: decisions[i].outcome,
      outcomeKind: decisions[i].outcomeKind,
    })
  }

  // Happy path: all pass
  rows.push({
    values: Array(n).fill('T'),
    outcome: happyOutcome,
    outcomeKind: 'happy',
  })

  return rows
}

// For each decision, produce a MC/DC pair:
//   rowFail: predicate[i] = F (guard fires) → early exit
//   rowPass: all T → happy path
function buildMcdcCases(
  decisions: DecisionPoint[],
  truthRows: TruthRow[],
  happyRow: TruthRow,
): McdcCase[] {
  return decisions.map((d, i) => ({
    predicate: d.predicate,
    index: i,
    rowFail: truthRows[i],
    rowPass: happyRow,
  }))
}
