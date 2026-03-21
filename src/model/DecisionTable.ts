// A single branching decision: "if (predicate) { ... }"
export type DecisionPoint = {
  index: number
  predicate: string
  line: number
}

// One row in the truth table
// values[i] = 'T' | 'F' | '*' (don't care)
export type TruthRow = {
  values: ('T' | 'F' | '*')[]
  outcome: string
  outcomeLabel: string
  outcomeKind: 'return' | 'throw' | 'happy'
  line: number
}

// A pair of rows demonstrating that one predicate independently affects the outcome
export type McdcCase = {
  predicate: string
  index: number
  rowFalse: TruthRow
  rowTrue: TruthRow
}

export type DecisionTable = {
  symbolName: string
  symbolKind: string
  decisions: DecisionPoint[]
  truthRows: TruthRow[]
  mcdcCases: McdcCase[]
  happyPath: TruthRow | null
}
