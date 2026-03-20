import type { FunctionReport } from '../model/FunctionReport.ts'
import type { DecisionTable } from '../model/DecisionTable.ts'
import { buildDecisionTable } from './buildDecisionTable.ts'

export type LlmSummary = {
  symbol: string
  kind: string
  file: string
  line: number
  complexity: number
  predicates: Array<{
    text: string
    kind: string
    context: string
    negated: boolean
  }>
  effects: Array<{
    kind: string
    sideEffect: string
    text: string
  }>
  decisionChain: Array<{
    predicate: string
    outcome: string
  }> | null
}

export function buildLlmSummary(report: FunctionReport): LlmSummary {
  const table: DecisionTable | null = buildDecisionTable(report as any)

  return {
    symbol: report.symbolName,
    kind: report.symbolKind,
    file: report.filePath,
    line: report.startLine,
    complexity: report.metrics.cyclomaticComplexity,
    predicates: (report.predicates ?? []).map(p => ({
      text: p.text,
      kind: p.kind,
      context: p.context,
      negated: p.negated,
    })),
    effects: (report.effects ?? []).filter(e => e.kind !== 'return' || !e.text.includes('return {')).concat(
      (report.effects ?? []).filter(e => e.kind === 'return').slice(0, 8)
    ).slice(0, 20).map(e => ({
      kind: e.kind,
      sideEffect: e.sideEffect,
      text: e.text.length > 80 ? e.text.slice(0, 77) + '...' : e.text,
    })),
    decisionChain: table
      ? table.decisions.map((d, i) => ({
          predicate: d.predicate,
          outcome: table.truthRows[i]?.outcome.slice(0, 80) ?? '',
        }))
      : null,
  }
}
