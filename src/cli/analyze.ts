import { Command } from 'commander'
import { analyzeSourceFile, analyzeSourceFileWithNodes } from '../analyzer/analyzeFunction.ts'
import { printAst } from '../analyzer/printAst.ts'
import { detectRuleCandidates } from '../analyzer/detectRuleCandidates.ts'
import { buildDecisionTable } from '../analyzer/buildDecisionTable.ts'
import { renderTestTemplate } from '../analyzer/renderTestTemplate.ts'
import type { FunctionReport } from '../model/FunctionReport.ts'
import type { RuleCandidate } from '../model/RuleCandidate.ts'
import type { DecisionTable } from '../model/DecisionTable.ts'

const program = new Command()

program
  .name('analyze')
  .description('Analyze TypeScript symbol complexity')
  .argument('<file>', 'TypeScript file to analyze')
  .option('--json', 'Output as raw JSON')
  .option('--top <n>', 'Show top N symbols by complexity', parseInt)
  .option('--min-complexity <n>', 'Filter symbols with cyclomaticComplexity >= N', parseInt)
  .option('--debug', 'Show AST tree for each symbol after metrics')
  .option('--debug-depth <n>', 'Max AST depth shown with --debug (default: 8)', parseInt)
  .option('--predicates', 'Show extracted atomic predicates for each symbol')
  .option('--effects', 'Show extracted effects (calls, assignments, returns) for each symbol')
  .option('--rule-candidates', 'Show rule engine candidate analysis across all symbols')
  .option('--decision-table', 'Show decision table (truth table + MC/DC) for each symbol')
  .option('--test-template', 'Generate vitest test templates from MC/DC cases')
  .parse()

const [file] = program.args
const opts = program.opts<{
  json?: boolean
  top?: number
  minComplexity?: number
  debug?: boolean
  debugDepth?: number
  predicates?: boolean
  effects?: boolean
  ruleCandidates?: boolean
  decisionTable?: boolean
  testTemplate?: boolean
}>()

if (opts.debug && opts.json) {
  console.error('--debug and --json cannot be used together')
  process.exit(1)
}
if (opts.predicates && opts.json) {
  console.error('--predicates and --json cannot be used together')
  process.exit(1)
}
if (opts.effects && opts.json) {
  console.error('--effects and --json cannot be used together')
  process.exit(1)
}
if (opts.ruleCandidates && opts.json) {
  console.error('--rule-candidates and --json cannot be used together')
  process.exit(1)
}

// --debug モードはノード付きで取得
const withNodes = opts.debug ? analyzeSourceFileWithNodes(file) : null
let reports: FunctionReport[]

try {
  if (withNodes) {
    reports = withNodes.map(({ node: _node, ...rest }) => rest)
  } else {
    reports = analyzeSourceFile(file)
  }
} catch (e) {
  console.error((e as Error).message)
  process.exit(1)
}

// Sort by cyclomaticComplexity descending
reports.sort((a, b) => b.metrics.cyclomaticComplexity - a.metrics.cyclomaticComplexity)
if (withNodes) {
  withNodes.sort((a, b) => b.metrics.cyclomaticComplexity - a.metrics.cyclomaticComplexity)
}

// Filter
if (opts.minComplexity !== undefined) {
  const pred = (r: FunctionReport) => r.metrics.cyclomaticComplexity >= opts.minComplexity!
  reports = reports.filter(pred)
  if (withNodes) {
    withNodes.splice(0, withNodes.length, ...withNodes.filter(pred))
  }
}

// Top N
if (opts.top !== undefined) {
  reports = reports.slice(0, opts.top)
  if (withNodes) withNodes.splice(opts.top)
}

if (opts.json) {
  console.log(JSON.stringify(reports, null, 2))
} else if (opts.debug && withNodes) {
  prettyPrintWithAst(withNodes, opts.debugDepth ?? 8)
} else if (opts.predicates) {
  prettyPrintWithPredicates(reports)
} else if (opts.effects) {
  prettyPrintWithEffects(reports)
} else if (opts.ruleCandidates) {
  prettyPrintRuleCandidates(detectRuleCandidates(reports))
} else if (opts.decisionTable) {
  prettyPrintDecisionTables(reports)
} else if (opts.testTemplate) {
  printTestTemplates(reports)
} else {
  prettyPrint(reports)
}

function prettyPrint(reports: FunctionReport[]): void {
  if (reports.length === 0) {
    console.log('No analyzable symbols found.')
    return
  }
  for (const r of reports) {
    const m = r.metrics
    console.log(`\n[${r.symbolKind}] ${r.symbolName}  (${r.filePath}:${r.startLine})`)
    console.log(`  CC=${m.cyclomaticComplexity}  nestDepth=${m.maxNestingDepth}  fnNest=${m.functionNestingDepth}  cbDepth=${m.callbackNestingDepth}`)
    console.log(`  if=${m.ifCount}  elseif=${m.elseIfCount}  switch=${m.switchCount}  ternary=${m.ternaryCount}  return=${m.returnCount}`)
    console.log(`  localFns=${m.localFunctionCount}  logicalOps=${m.logicalOperatorCount ?? 0}`)
    console.log(`  negations=${m.negationCount ?? 0}  atomicConds=${m.atomicConditionCount ?? 0}  maxCondDepth=${m.maxConditionDepth ?? 0}`)
  }
  console.log()
}

function prettyPrintWithPredicates(reports: FunctionReport[]): void {
  if (reports.length === 0) {
    console.log('No analyzable symbols found.')
    return
  }
  const DIVIDER = '─'.repeat(72)
  for (const r of reports) {
    const m = r.metrics
    console.log(`\n${DIVIDER}`)
    console.log(`[${r.symbolKind}] ${r.symbolName}  (${r.filePath}:${r.startLine})`)
    console.log(`  CC=${m.cyclomaticComplexity}  negations=${m.negationCount ?? 0}  atomicConds=${m.atomicConditionCount ?? 0}  maxCondDepth=${m.maxConditionDepth ?? 0}`)
    const preds = r.predicates ?? []
    if (preds.length === 0) {
      console.log('  (no predicates)')
    } else {
      for (const p of preds) {
        const neg = p.negated ? '!' : ' '
        console.log(`  ${neg} [${p.kind.padEnd(11)}] [${p.context.padEnd(6)}] :${p.line}  ${p.text}`)
      }
    }
  }
  console.log(`\n${DIVIDER}`)
}

function prettyPrintDecisionTables(reports: FunctionReport[]): void {
  const DIVIDER = '─'.repeat(72)
  let found = 0
  for (const r of reports) {
    const table = buildDecisionTable(r)
    if (!table) continue
    found++
    console.log(`\n${DIVIDER}`)
    console.log(`[${r.symbolKind}] ${r.symbolName}  :${r.startLine}  (${table.decisions.length} decisions)`)

    // Header
    const header = table.decisions.map((d, i) => `P${i + 1}`.padEnd(4)).join(' ') + '  outcome'
    const predLine = table.decisions.map((d, i) => `P${i + 1}=${d.predicate.slice(0, 20).padEnd(20)}`).join('  ')
    console.log(`\n  ${predLine}`)
    console.log(`\n  ${'P'.padEnd(4).repeat(0)}${header}`)
    console.log(`  ${'─'.repeat(header.length)}`)

    for (const row of table.truthRows) {
      const vals = row.values.map(v => v.padEnd(4)).join(' ')
      const out = row.outcome.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim()
      const outShort = out.length > 40 ? out.slice(0, 37) + '...' : out
      const tag = row.outcomeKind === 'happy' ? ' ← happy path' : ''
      console.log(`  ${vals}  ${outShort}${tag}`)
    }

    console.log(`\n  [MC/DC cases: ${table.mcdcCases.length}]`)
    for (const mc of table.mcdcCases) {
      const pred = mc.predicate.length > 40 ? mc.predicate.slice(0, 37) + '...' : mc.predicate
      console.log(`  P${mc.index + 1}: "${pred}"  →  fail row=${mc.rowFail.values.join('')}  pass row=${mc.rowPass.values.join('')}`)
    }
  }
  if (found === 0) console.log('No decision tables could be built.')
  else console.log(`\n${DIVIDER}`)
}

function printTestTemplates(reports: FunctionReport[]): void {
  const DIVIDER = '─'.repeat(72)
  let found = 0
  for (const r of reports) {
    const table = buildDecisionTable(r)
    if (!table) continue
    found++
    console.log(`\n${DIVIDER}`)
    console.log(`// ${r.filePath}:${r.startLine}  [${r.symbolKind}] ${r.symbolName}`)
    console.log(`${DIVIDER}\n`)
    console.log(renderTestTemplate(table))
  }
  if (found === 0) console.log('No test templates could be generated.')
}

function prettyPrintRuleCandidates(candidates: RuleCandidate[]): void {
  if (candidates.length === 0) {
    console.log('No rule candidate signals found.')
    return
  }
  const DIVIDER = '─'.repeat(72)
  console.log(`\nRule Candidate Analysis  (${candidates.length} signals found)\n${DIVIDER}`)
  for (const c of candidates) {
    console.log(`\n[score=${c.score}] [${c.symbolKind}] ${c.symbolName}  :${c.startLine}`)
    console.log(`  signals: ${c.signals.join(', ')}`)
    for (const d of c.details) {
      console.log(`  • ${d}`)
    }
    if (c.similarFunctions.length > 0) {
      console.log(`  similar:`)
      for (const s of c.similarFunctions) {
        console.log(`    ${s.symbolName}  overlap=${Math.round(s.overlapRatio * 100)}%  shared: ${s.sharedPredicates.slice(0, 3).join(', ')}`)
      }
    }
  }
  console.log(`\n${DIVIDER}`)
}

function prettyPrintWithEffects(reports: FunctionReport[]): void {
  if (reports.length === 0) {
    console.log('No analyzable symbols found.')
    return
  }
  const DIVIDER = '─'.repeat(72)
  for (const r of reports) {
    const m = r.metrics
    console.log(`\n${DIVIDER}`)
    console.log(`[${r.symbolKind}] ${r.symbolName}  (${r.filePath}:${r.startLine})`)
    console.log(`  CC=${m.cyclomaticComplexity}  return=${m.returnCount}`)
    const effs = r.effects ?? []
    if (effs.length === 0) {
      console.log('  (no effects)')
    } else {
      for (const e of effs) {
        const label = `[${e.kind.padEnd(10)}][${e.sideEffect.padEnd(11)}]`
        const text = e.text.length > 60 ? e.text.slice(0, 57) + '...' : e.text
        console.log(`  ${label} :${e.line}  ${text}`)
      }
    }
  }
  console.log(`\n${DIVIDER}`)
}

function prettyPrintWithAst(
  items: (FunctionReport & { node: import('../analyzer/collectFunctions.ts').AnalyzableNode })[],
  maxDepth: number,
): void {
  if (items.length === 0) {
    console.log('No analyzable symbols found.')
    return
  }
  const DIVIDER = '─'.repeat(72)
  for (const r of items) {
    const m = r.metrics
    console.log(`\n${DIVIDER}`)
    console.log(`[${r.symbolKind}] ${r.symbolName}  (${r.filePath}:${r.startLine})`)
    console.log(`  CC=${m.cyclomaticComplexity}  nestDepth=${m.maxNestingDepth}  fnNest=${m.functionNestingDepth}  cbDepth=${m.callbackNestingDepth}`)
    console.log(`  if=${m.ifCount}  elseif=${m.elseIfCount}  switch=${m.switchCount}  ternary=${m.ternaryCount}  return=${m.returnCount}`)
    console.log(`  localFns=${m.localFunctionCount}  logicalOps=${m.logicalOperatorCount ?? 0}`)
    console.log(`  negations=${m.negationCount ?? 0}  atomicConds=${m.atomicConditionCount ?? 0}  maxCondDepth=${m.maxConditionDepth ?? 0}`)
    console.log()
    console.log('  [AST]  markers: [CC+1] cyclomatic +1 / [nest] nesting depth +1 / [fn] nested function')
    const ast = printAst(r.node, maxDepth)
    for (const line of ast.split('\n')) {
      console.log(`  ${line}`)
    }
  }
  console.log(`\n${DIVIDER}`)
}
