import type { FunctionMetrics } from './FunctionMetrics.ts'
import type { AtomicPredicate } from './Predicate.ts'
import type { Effect } from './Effect.ts'

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
  predicates?: AtomicPredicate[]
  effects?: Effect[]
  // hints?: AnalysisHints  // M3+
}
