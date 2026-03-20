import type { FunctionMetrics } from './FunctionMetrics.ts'

export type SymbolKind = 'function' | 'variableFunction' | 'method' | 'class'

export type FunctionReport = {
  symbolName: string
  symbolKind: SymbolKind
  className?: string
  memberName?: string
  functionName: string
  filePath: string
  startLine: number
  metrics: FunctionMetrics
  // 将来追加予定（今は実装しない）
  // predicates?: AtomicPredicate[]
  // effects?: Effect[]
  // hints?: AnalysisHints
}
