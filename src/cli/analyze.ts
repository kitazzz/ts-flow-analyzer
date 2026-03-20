import { Command } from 'commander'
import { analyzeSourceFile, analyzeSourceFileWithNodes } from '../analyzer/analyzeFunction.ts'
import { printAst } from '../analyzer/printAst.ts'
import type { FunctionReport } from '../model/FunctionReport.ts'

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
  .parse()

const [file] = program.args
const opts = program.opts<{
  json?: boolean
  top?: number
  minComplexity?: number
  debug?: boolean
  debugDepth?: number
}>()

if (opts.debug && opts.json) {
  console.error('--debug and --json cannot be used together')
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
  }
  console.log()
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
    console.log()
    console.log('  [AST]  markers: [CC+1] cyclomatic +1 / [nest] nesting depth +1 / [fn] nested function')
    const ast = printAst(r.node, maxDepth)
    for (const line of ast.split('\n')) {
      console.log(`  ${line}`)
    }
  }
  console.log(`\n${DIVIDER}`)
}
