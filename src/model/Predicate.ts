export type PredicateKind =
  | 'comparison'   // a > b, a === b, a !== b, etc.
  | 'nullCheck'    // a === null/undefined, a == null
  | 'typeCheck'    // typeof x === '...', x instanceof Y
  | 'call'         // fn(), obj.method() used as condition
  | 'truthiness'   // plain identifier/property used as boolean (if (user))
  | 'negation'     // !expr where expr is a leaf
  | 'other'        // fallback

export type DecisionContext =
  | 'if'
  | 'elseif'
  | 'ternary'
  | 'while'
  | 'doWhile'
  | 'for'
  | 'case'

export type AtomicPredicate = {
  text: string            // raw source text of the atomic expression
  negated: boolean        // directly wrapped in ! at extraction point
  kind: PredicateKind
  context: DecisionContext
  line: number
}
