export type FunctionMetrics = {
  ifCount: number
  elseIfCount: number
  switchCount: number
  ternaryCount: number
  returnCount: number
  maxNestingDepth: number
  cyclomaticComplexity: number
  localFunctionCount: number
  functionNestingDepth: number
  callbackNestingDepth: number
  logicalOperatorCount?: number
}
