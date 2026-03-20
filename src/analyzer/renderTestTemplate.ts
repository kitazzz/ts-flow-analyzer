import type { DecisionTable, DecisionPoint } from '../model/DecisionTable.ts'

export function renderTestTemplate(table: DecisionTable): string {
  const { symbolName, decisions, mcdcCases, happyPath } = table
  const lines: string[] = []

  lines.push(`import { describe, it, expect } from 'vitest'`)
  lines.push(`// import { ${symbolName} } from './${symbolName}'`)
  lines.push(``)
  lines.push(`// Auto-generated test template from MC/DC analysis`)
  lines.push(`// Decisions extracted: ${decisions.length}`)
  lines.push(`// Each test case demonstrates one predicate independently affecting the outcome`)
  lines.push(``)
  lines.push(`describe('${symbolName}', () => {`)

  // One test per MC/DC case
  for (const mc of mcdcCases) {
    const d = decisions[mc.index]
    const guardDesc = describeGuard(d)
    const falseOutcome = truncate(mc.rowFalse.outcome, 60)
    const trueOutcome = truncate(mc.rowTrue.outcome, 60)

    lines.push(``)
    lines.push(`  // MC/DC case ${mc.index + 1}: predicate "${d.predicate}"`)
    lines.push(`  it('should distinguish ${guardDesc}', () => {`)
    lines.push(`    // Arrange false-case: ${d.predicate} = false`)
    lines.push(`    //   row: ${mc.rowFalse.values.join('')}  -> ${mc.rowFalse.outcomeLabel}`)
    lines.push(`    // Arrange true-case:  ${d.predicate} = true`)
    lines.push(`    //   row: ${mc.rowTrue.values.join('')}  -> ${mc.rowTrue.outcomeLabel}`)
    lines.push(`    //   other predicates: ${describeOtherPredicates(decisions, mc.rowTrue.values, mc.index)}`)
    lines.push(`    const input = buildInput({ /* fill branch-specific values */ })`)
    lines.push(``)
    lines.push(`    // Act`)
    lines.push(`    // const result = await sut.execute(input)`)
    lines.push(``)
    lines.push(`    // Assert false-case: expects → ${falseOutcome}`)
    lines.push(`    // Assert true-case:  expects → ${trueOutcome}`)
    lines.push(`    // expect(result).toMatchObject({ ... })`)
    lines.push(`  })`)
  }

  // Happy path test
  if (happyPath) {
    lines.push(``)
    lines.push(`  // Representative success path`)
    lines.push(`  it('should succeed on the representative success path', () => {`)
    lines.push(`    // Arrange: ${describeRow(decisions, happyPath.values)}`)
    lines.push(`    const input = buildInput({ /* all valid */ })`)
    lines.push(``)
    lines.push(`    // Act`)
    lines.push(`    // const result = await sut.execute(input)`)
    lines.push(``)
    lines.push(`    // Assert: expects → ${happyPath.outcomeLabel} (${truncate(happyPath.outcome, 60)})`)
    lines.push(`    // expect(result).toMatchObject({ ... })`)
    lines.push(`  })`)
  }

  lines.push(`})`)
  lines.push(``)
  lines.push(`function buildInput(overrides: Record<string, unknown> = {}) {`)
  lines.push(`  return {`)
  lines.push(`    // TODO: fill in default valid input`)
  lines.push(`    ...overrides,`)
  lines.push(`  }`)
  lines.push(`}`)

  return lines.join('\n')
}

function describeGuard(d: DecisionPoint): string {
  return `predicate "${d.predicate}" switching between false and true`
}

function describeOtherPredicates(
  decisions: DecisionPoint[],
  values: ('T' | 'F' | '*')[],
  skipIndex: number,
): string {
  const parts = decisions
    .map((decision, index) => {
      if (index === skipIndex) return null
      const value = values[index]
      if (value === '*') return null
      return `${decision.predicate}=${value === 'T' ? 'true' : 'false'}`
    })
    .filter((part): part is string => Boolean(part))

  if (parts.length === 0) return 'none'
  return parts
    .join(', ')
}

function describeRow(
  decisions: DecisionPoint[],
  values: ('T' | 'F' | '*')[],
): string {
  const parts = decisions
    .map((decision, index) => {
      const value = values[index]
      if (value === '*') return null
      return `${decision.predicate}=${value === 'T' ? 'true' : 'false'}`
    })
    .filter((part): part is string => Boolean(part))

  if (parts.length === 0) return 'no explicit predicates'
  return parts.join(', ')
}

function truncate(text: string, max: number): string {
  const s = text.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim()
  return s.length > max ? s.slice(0, max - 3) + '...' : s
}
