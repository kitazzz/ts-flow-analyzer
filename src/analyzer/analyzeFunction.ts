import { Node, SourceFile, SyntaxKind } from 'ts-morph'
import type { FunctionReport } from '../model/FunctionReport.ts'
import { loadSourceFile } from './project.ts'
import { collectFunctions, type AnalyzableNode } from './collectFunctions.ts'
import { analyzeBasicComplexity } from './analyzeBasicComplexity.ts'
import { analyzeMaxNestingDepth } from './analyzeNesting.ts'
import { analyzeCyclomaticComplexity } from './analyzeCyclomatic.ts'
import { analyzeFunctionNesting } from './analyzeFunctionNesting.ts'
import { analyzeConditionComplexity } from './analyzeCondition.ts'
import { extractPredicates } from './extractPredicates.ts'
import { extractEffects } from './extractEffects.ts'

export type FunctionReportWithNode = FunctionReport & { node: AnalyzableNode }

function countLogicalOperators(fnNode: Node): number {
  let count = 0
  for (const node of fnNode.getDescendants()) {
    const kind = node.getKind()
    if (kind === SyntaxKind.AmpersandAmpersandToken || kind === SyntaxKind.BarBarToken) {
      count++
    }
  }
  return count
}

export function analyzeSourceFile(filePath: string): FunctionReport[] {
  return analyzeSourceFileWithNodes(filePath).map(({ node: _node, ...rest }) => rest)
}

export function analyzeSourceFileWithNodes(filePath: string): FunctionReportWithNode[] {
  const sourceFile = loadSourceFile(filePath)
  return collectFunctions(sourceFile).map(({ symbolName, symbolKind, className, memberName, node, filePath, startLine }) => {
    const basic = analyzeBasicComplexity(node)
    const maxNestingDepth = analyzeMaxNestingDepth(node)
    const cyclomaticComplexity = analyzeCyclomaticComplexity(node)
    const nesting = analyzeFunctionNesting(node)
    const logicalOperatorCount = countLogicalOperators(node)
    const condition = analyzeConditionComplexity(node)
    const predicates = extractPredicates(node)
    const effects = extractEffects(node)

    return {
      symbolName,
      symbolKind,
      className,
      memberName,
      functionName: symbolName,
      filePath,
      startLine,
      node,
      metrics: {
        ...basic,
        maxNestingDepth,
        cyclomaticComplexity,
        ...nesting,
        logicalOperatorCount,
        ...condition,
      },
      predicates,
      effects,
    }
  })
}
