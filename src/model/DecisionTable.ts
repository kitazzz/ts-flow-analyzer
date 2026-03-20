// A single guard decision: "if (predicate) → outcome"
export type DecisionPoint = {
  index: number
  predicate: string       // atomic predicate text
  negated: boolean        // true if the guard fires when predicate is falsy (if !x → fail)
  predicateKind: string
  outcome: string         // return/throw text
  outcomeKind: 'return' | 'throw'
  line: number
}

// One row in the truth table
// values[i] = 'T' | 'F' | '*' (don't care)
export type TruthRow = {
  values: ('T' | 'F' | '*')[]
  outcome: string
  outcomeKind: 'return' | 'throw' | 'happy'
}

// A pair of rows demonstrating that one predicate independently affects the outcome
export type McdcCase = {
  predicate: string
  index: number
  rowFail: TruthRow    // this predicate triggers the guard → early exit
  rowPass: TruthRow    // all guards pass → happy path
}

export type DecisionTable = {
  symbolName: string
  symbolKind: string
  decisions: DecisionPoint[]
  truthRows: TruthRow[]
  mcdcCases: McdcCase[]
  happyPath: TruthRow | null
}
