export type RuleCandidateSignal =
  | 'repeatedPredicate'    // same predicate text appears in 2+ other functions
  | 'tenantBranch'         // role/type/plan/tenant/tier comparison detected
  | 'decisionConvergence'  // multiple branches returning same shape (guard/result pattern)
  | 'highComplexity'       // CC >= 5
  | 'similarFunction'      // shares 50%+ predicates with another function in the file

export type SimilarFunction = {
  symbolName: string
  sharedPredicates: string[]
  overlapRatio: number  // shared / min(a, b)
}

export type RuleCandidate = {
  symbolName: string
  symbolKind: string
  filePath: string
  startLine: number
  score: number
  signals: RuleCandidateSignal[]
  details: string[]
  similarFunctions: SimilarFunction[]
}
