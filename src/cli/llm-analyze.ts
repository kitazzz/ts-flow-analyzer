import { Command } from 'commander'
import { query } from '@anthropic-ai/claude-agent-sdk'
import { analyzeSourceFile } from '../analyzer/analyzeFunction.ts'
import { buildLlmSummary } from '../analyzer/buildLlmSummary.ts'
import type { FunctionReport } from '../model/FunctionReport.ts'
import path from 'path'

const program = new Command()

program
  .name('llm-analyze')
  .description('Claude Code agent で business rule を分析する')
  .argument('<file>', 'TypeScript file to analyze')
  .option('--symbol <name>', '特定のシンボル名を対象にする')
  .option('--min-complexity <n>', 'cyclomaticComplexity >= N のシンボルのみ', parseInt)
  .option('--top <n>', '上位 N 件を解析', parseInt)
  .parse()

const [file] = program.args
const opts = program.opts<{
  symbol?: string
  minComplexity?: number
  top?: number
}>()

let reports: FunctionReport[]
try {
  reports = analyzeSourceFile(file)
} catch (e) {
  console.error((e as Error).message)
  process.exit(1)
}

reports.sort((a, b) => b.metrics.cyclomaticComplexity - a.metrics.cyclomaticComplexity)

if (opts.symbol) {
  reports = reports.filter(r => r.symbolName.includes(opts.symbol!))
} else {
  if (opts.minComplexity !== undefined) {
    reports = reports.filter(r => r.metrics.cyclomaticComplexity >= opts.minComplexity!)
  }
  if (opts.top !== undefined) {
    reports = reports.slice(0, opts.top)
  }
}

if (reports.length === 0) {
  console.log('No symbols matched the filter.')
  process.exit(0)
}

const DIVIDER = '─'.repeat(72)
const absFile = path.resolve(file)
const cwd = path.dirname(absFile)

for (const report of reports) {
  const summary = buildLlmSummary(report)

  console.log(`\n${DIVIDER}`)
  console.log(`[${report.symbolKind}] ${report.symbolName}  :${report.startLine}  CC=${report.metrics.cyclomaticComplexity}`)
  console.log(`Spawning Claude Code agent...\n`)

  const prompt = buildAgentPrompt(summary, absFile)

  for await (const message of query({
    prompt,
    options: {
      cwd,
      allowedTools: ['Read', 'Grep', 'Glob'],
      maxTurns: 8,
    },
  })) {
    if ('result' in message) {
      console.log(message.result)
    }
  }
}

console.log(`\n${DIVIDER}`)

function buildAgentPrompt(summary: ReturnType<typeof buildLlmSummary>, filePath: string): string {
  const lines: string[] = []

  lines.push(`You are a domain-driven design expert and business rule analyst.`)
  lines.push(`Analyze the TypeScript function described below and read the source file to get full context.`)
  lines.push(``)
  lines.push(`## Source`)
  lines.push(`File: ${filePath}`)
  lines.push(`Symbol: ${summary.symbol}  (${summary.kind}, line ${summary.line})`)
  lines.push(`Cyclomatic Complexity: ${summary.complexity}`)
  lines.push(``)

  if (summary.predicates.length > 0) {
    lines.push(`## Extracted Conditions`)
    for (const p of summary.predicates) {
      const neg = p.negated ? '[!] ' : '    '
      lines.push(`${neg}${p.text}  (${p.kind}, ctx=${p.context})`)
    }
    lines.push(``)
  }

  if (summary.effects.length > 0) {
    lines.push(`## Extracted Effects`)
    for (const e of summary.effects) {
      lines.push(`  [${e.kind}/${e.sideEffect}]  ${e.text}`)
    }
    lines.push(``)
  }

  if (summary.decisionChain) {
    lines.push(`## Decision Chain`)
    for (let i = 0; i < summary.decisionChain.length; i++) {
      const d = summary.decisionChain[i]
      lines.push(`  ${i + 1}. if (${d.predicate}) → ${d.outcome}`)
    }
    lines.push(``)
  }

  lines.push(`## Your Task`)
  lines.push(``)
  lines.push(`Read the source file to understand the full context, then provide:`)
  lines.push(``)
  lines.push(`1. **Facts** — domain entities/values used as conditions. Give each a clean domain name.`)
  lines.push(`   Format: \`factName\`: description`)
  lines.push(``)
  lines.push(`2. **Decisions** — distinct business rules. Name each decision and describe when it applies.`)
  lines.push(`   Format: \`decisionName\`: condition → outcome`)
  lines.push(``)
  lines.push(`3. **Actions** — side effects with domain names and classification.`)
  lines.push(`   Format: \`actionName\`: description  [class]`)
  lines.push(``)
  lines.push(`4. **Rule Extraction Opportunities** — what could become a reusable rule/policy?`)
  lines.push(`   Suggest: rule name, condition, where to put it (function / service / policy class).`)
  lines.push(``)
  lines.push(`5. **Design Notes** — responsibilities, naming suggestions, split recommendations.`)
  lines.push(``)
  lines.push(`Use the real variable and method names from the code. Be concise and practical.`)

  return lines.join('\n')
}
