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
    const failOutcome = truncate(d.outcome, 60)

    lines.push(``)
    lines.push(`  // MC/DC case ${mc.index + 1}: predicate "${d.predicate}"`)
    lines.push(`  it('should ${d.outcomeKind} early when ${guardDesc}', () => {`)
    lines.push(`    // Arrange: ${d.predicate} is falsy`)
    lines.push(`    //   preconditions: ${describePassedGuards(decisions, mc.index)}`)
    lines.push(`    const input = buildInput({ /* ${d.predicate} = false/null/undefined */ })`)
    lines.push(``)
    lines.push(`    // Act`)
    lines.push(`    // const result = await sut.execute(input)`)
    lines.push(``)
    lines.push(`    // Assert: expects → ${failOutcome}`)
    lines.push(`    // expect(result).toMatchObject({ ... })`)
    lines.push(`  })`)
  }

  // Happy path test
  if (happyPath) {
    lines.push(``)
    lines.push(`  // Happy path: all ${decisions.length} guards pass`)
    lines.push(`  it('should succeed when all conditions are met', () => {`)
    lines.push(`    // Arrange: ${decisions.map(d => `${d.predicate} = truthy`).join(', ')}`)
    lines.push(`    const input = buildInput({ /* all valid */ })`)
    lines.push(``)
    lines.push(`    // Act`)
    lines.push(`    // const result = await sut.execute(input)`)
    lines.push(``)
    lines.push(`    // Assert: expects → ${truncate(happyPath.outcome, 60)}`)
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
  if (d.negated) return `${d.predicate} is falsy`
  return `${d.predicate} is not satisfied`
}

function describePassedGuards(decisions: DecisionPoint[], upTo: number): string {
  if (upTo === 0) return 'none'
  return decisions
    .slice(0, upTo)
    .map(d => `${d.predicate}=ok`)
    .join(', ')
}

function truncate(text: string, max: number): string {
  const s = text.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim()
  return s.length > max ? s.slice(0, max - 3) + '...' : s
}
