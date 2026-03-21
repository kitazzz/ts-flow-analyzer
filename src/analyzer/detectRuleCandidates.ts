import type { FunctionReport } from '../model/FunctionReport.ts'
import type { RuleCandidate, RuleCandidateSignal, SimilarFunction } from '../model/RuleCandidate.ts'

// Tenant/role/flag-like identifiers that suggest multi-tenant branching
const TENANT_PATTERN = /\b(role|type|plan|tier|tenant|company|flag|feature|mode|env|environment|permission|scope)\b/i

// Signal weights for scoring
const SIGNAL_WEIGHT: Record<RuleCandidateSignal, number> = {
  repeatedPredicate:   3,
  tenantBranch:        2,
  decisionConvergence: 2,
  highComplexity:      1,
  similarFunction:     2,
}

const SIMILARITY_THRESHOLD = 0.5   // 50% predicate overlap
const HIGH_CC_THRESHOLD = 5
const CONVERGENCE_MIN_RETURNS = 3  // at least N returns with same shape

export function detectRuleCandidates(reports: FunctionReport[]): RuleCandidate[] {
  // --- Step 1: build global predicate → functions map ---
  const predicateIndex = new Map<string, string[]>() // predicate text → [symbolName]

  for (const r of reports) {
    for (const p of r.predicates ?? []) {
      const key = normalizePredicateText(p.text)
      if (!predicateIndex.has(key)) predicateIndex.set(key, [])
      predicateIndex.get(key)!.push(r.symbolName)
    }
  }

  // Predicates that appear in 2+ distinct functions
  const repeatedPredicates = new Map<string, string[]>() // text → [symbolNames]
  for (const [text, symbols] of predicateIndex) {
    const uniq = [...new Set(symbols)]
    if (uniq.length >= 2) repeatedPredicates.set(text, uniq)
  }

  // --- Step 2: per-function predicate sets (normalized) for similarity ---
  const predicateSets = new Map<string, Set<string>>()
  for (const r of reports) {
    const s = new Set((r.predicates ?? []).map(p => normalizePredicateText(p.text)))
    predicateSets.set(r.symbolName, s)
  }

  // --- Step 3: evaluate each function ---
  const candidates: RuleCandidate[] = []

  for (const r of reports) {
    const signals: RuleCandidateSignal[] = []
    const details: string[] = []

    // Signal: highComplexity
    if (r.metrics.cyclomaticComplexity >= HIGH_CC_THRESHOLD) {
      signals.push('highComplexity')
      details.push(`CC=${r.metrics.cyclomaticComplexity}`)
    }

    // Signal: repeatedPredicate
    const myNormalized = predicateSets.get(r.symbolName) ?? new Set<string>()
    const myRepeated: string[] = []
    for (const pred of myNormalized) {
      if (repeatedPredicates.has(pred)) {
        const others = repeatedPredicates.get(pred)!.filter(s => s !== r.symbolName)
        if (others.length > 0) myRepeated.push(pred)
      }
    }
    if (myRepeated.length > 0) {
      signals.push('repeatedPredicate')
      details.push(`Repeated predicates (${myRepeated.length}): ${myRepeated.slice(0, 3).join(', ')}`)
    }

    // Signal: tenantBranch
    const tenantPreds = (r.predicates ?? []).filter(p => TENANT_PATTERN.test(p.text))
    if (tenantPreds.length > 0) {
      signals.push('tenantBranch')
      details.push(`Tenant/role branches (${tenantPreds.length}): ${tenantPreds.slice(0, 3).map(p => p.text).join(', ')}`)
    }

    // Signal: decisionConvergence
    // Look for functions where most returns follow a consistent shape (e.g., { ok: ... } or { success: ... })
    const returnEffects = (r.effects ?? []).filter(e => e.kind === 'return')
    if (returnEffects.length >= CONVERGENCE_MIN_RETURNS) {
      const shape = detectReturnShape(returnEffects.map(e => e.text))
      if (shape) {
        signals.push('decisionConvergence')
        details.push(`Convergent return shape: ${shape} (${returnEffects.length} returns)`)
      }
    }

    // Signal: similarFunction (pairwise)
    const similarFunctions: SimilarFunction[] = []
    if (myNormalized.size > 0) {
      for (const other of reports) {
        if (other.symbolName === r.symbolName) continue
        const otherSet = predicateSets.get(other.symbolName) ?? new Set<string>()
        if (otherSet.size === 0) continue
        const shared = [...myNormalized].filter(p => otherSet.has(p))
        const ratio = shared.length / Math.min(myNormalized.size, otherSet.size)
        if (ratio >= SIMILARITY_THRESHOLD) {
          similarFunctions.push({
            symbolName: other.symbolName,
            sharedPredicates: shared,
            overlapRatio: Math.round(ratio * 100) / 100,
          })
        }
      }
    }
    if (similarFunctions.length > 0) {
      signals.push('similarFunction')
      details.push(`Similar functions: ${similarFunctions.map(s => `${s.symbolName} (${Math.round(s.overlapRatio * 100)}%)`).join(', ')}`)
    }

    if (signals.length === 0) continue  // no signals → skip

    const score = signals.reduce((s, sig) => s + SIGNAL_WEIGHT[sig], 0)

    candidates.push({
      symbolName: r.symbolName,
      symbolKind: r.symbolKind,
      filePath: r.filePath,
      startLine: r.startLine,
      score,
      signals,
      details,
      similarFunctions,
    })
  }

  return candidates.sort((a, b) => b.score - a.score)
}

// Normalize predicate text for comparison: lowercase, collapse whitespace, strip quotes
function normalizePredicateText(text: string): string {
  return text
    .toLowerCase()
    .replace(/['"]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
}

// Detect if returns share a common shape pattern like { ok: ... } or { success: ... }
function detectReturnShape(returnTexts: string[]): string | null {
  const shapes = [
    { pattern: /return\s*\{\s*ok\s*:/, label: '{ ok: ... }' },
    { pattern: /return\s*\{\s*success\s*:/, label: '{ success: ... }' },
    { pattern: /return\s*\{\s*error\s*:/, label: '{ error: ... }' },
    { pattern: /return\s*\{\s*result\s*:/, label: '{ result: ... }' },
    { pattern: /return\s*(null|undefined|false|true)\b/, label: 'null/bool return' },
  ]

  for (const { pattern, label } of shapes) {
    const matchCount = returnTexts.filter(t => pattern.test(t)).length
    // At least half of returns match this shape
    if (matchCount >= CONVERGENCE_MIN_RETURNS && matchCount / returnTexts.length >= 0.5) {
      return label
    }
  }
  return null
}
