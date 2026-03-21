import type { AtomicPredicate, PredicateKind } from '../model/Predicate.ts'

type Names = { name: string; trueMeaning: string; falseMeaning: string }

export function normalizePredicates(predicates: AtomicPredicate[]): AtomicPredicate[] {
  return predicates.map(p => {
    try {
      const { name, trueMeaning, falseMeaning } = deriveNames(p)
      return { ...p, normalizedName: name, trueMeaning, falseMeaning }
    } catch {
      return p  // keep raw on failure — normalization is best-effort
    }
  })
}

// ─── Dispatch ────────────────────────────────────────────────────────────────

function deriveNames(p: AtomicPredicate): Names {
  switch (p.kind) {
    case 'truthiness': return deriveTruthiness(p.text, p.negated)
    case 'nullCheck':  return deriveNullCheck(p.text, p.negated)
    case 'comparison': return deriveComparison(p.text, p.negated)
    case 'call':       return deriveCall(p.text, p.negated)
    case 'negation':   return deriveTruthiness(stripLeadingNot(p.text), !p.negated)
    default:           return fallback(p.text)
  }
}

// ─── Per-kind derivers ────────────────────────────────────────────────────────

function deriveTruthiness(text: string, negated: boolean): Names {
  const base = pathToIdent(text)
  if (negated) {
    return {
      name: base + 'Missing',
      trueMeaning: `${text} is missing / falsy`,
      falseMeaning: `${text} exists`,
    }
  }
  return {
    name: base + 'Exists',
    trueMeaning: `${text} exists`,
    falseMeaning: `${text} is missing / falsy`,
  }
}

function deriveNullCheck(text: string, negated: boolean): Names {
  // e.g. "order === null", "user !== undefined"
  const m = text.match(/^(.+?)\s*(===|!==|==|!=)\s*(null|undefined)/)
  const subject = m ? m[1] : text
  const isNullOp = m ? (m[2] === '===' || m[2] === '==') : true
  const base = pathToIdent(subject)

  // negated + isNullOp: `!(order === null)` → orderNotNull
  const resultNegated = negated ? !isNullOp : isNullOp
  if (resultNegated) {
    return {
      name: base + 'Null',
      trueMeaning: `${subject} is null/undefined`,
      falseMeaning: `${subject} is not null`,
    }
  }
  return {
    name: base + 'NotNull',
    trueMeaning: `${subject} is not null`,
    falseMeaning: `${subject} is null/undefined`,
  }
}

function deriveComparison(text: string, negated: boolean): Names {
  // Parse: left OP right
  const m = text.match(/^(.+?)\s*(===|!==|==|!=|>=|<=|>|<)\s*(.+)$/)
  if (!m) return fallback(text)

  const [, rawLeft, op, rawRight] = m
  const left  = rawLeft.trim()
  const right = rawRight.trim()

  // Role / status string literal comparison
  // e.g. `operator.role !== 'admin'`
  const stringLiteral = right.match(/^['"](.+)['"]$/)
  if (stringLiteral) {
    const value = stringLiteral[1]
    const base = pathToIdent(left)
    const valueId = capitalize(camelize(value))

    const notOp = op === '!==' || op === '!='
    const effectiveNot = negated ? !notOp : notOp
    if (effectiveNot) {
      return {
        name: base + 'Not' + valueId,
        trueMeaning: `${left} is not '${value}'`,
        falseMeaning: `${left} is '${value}'`,
      }
    }
    return {
      name: base + 'Is' + valueId,
      trueMeaning: `${left} is '${value}'`,
      falseMeaning: `${left} is not '${value}'`,
    }
  }

  // .length comparison
  if (left.endsWith('.length')) {
    const subject = pathToIdent(left.replace(/\.length$/, ''))
    const gtOp = op === '>' || op === '>='
    const effectiveGt = negated ? !gtOp : gtOp
    if (right === '0') {
      return effectiveGt
        ? { name: subject + 'NonEmpty', trueMeaning: `${left} > 0`, falseMeaning: `${left} is 0` }
        : { name: subject + 'Empty',    trueMeaning: `${left} is 0`, falseMeaning: `${left} > 0` }
    }
    return effectiveGt
      ? { name: subject + 'LengthAbove' + camelize(right), trueMeaning: `${left} > ${right}`, falseMeaning: `${left} <= ${right}` }
      : { name: subject + 'LengthBelow' + camelize(right), trueMeaning: `${left} <= ${right}`, falseMeaning: `${left} > ${right}` }
  }

  // Numeric threshold comparison
  // e.g. `total >= HIGH_VALUE_THRESHOLD`, `item.quantity <= 0`, `item.unitPrice < 0`
  const numericRhs = right.match(/^[\d.]+$/) || right.match(/^[A-Z_]+$/)
  if (numericRhs) {
    const base = pathToIdent(left)
    const eop = negated ? negateOp(op) : op  // effective operator after negation
    const isGt = eop === '>' || eop === '>='
    const isStrict = eop === '>' || eop === '<'

    // Zero threshold → semantic names (Positive / NonNegative / Negative / ZeroOrBelow)
    if (right === '0') {
      if (isGt) {
        return isStrict
          ? { name: base + 'Positive',    trueMeaning: `${left} > 0`,  falseMeaning: `${left} <= 0` }
          : { name: base + 'NonNegative', trueMeaning: `${left} >= 0`, falseMeaning: `${left} < 0`  }
      } else {
        return isStrict
          ? { name: base + 'Negative',    trueMeaning: `${left} < 0`,  falseMeaning: `${left} >= 0` }
          : { name: base + 'ZeroOrBelow', trueMeaning: `${left} <= 0`, falseMeaning: `${left} > 0`  }
      }
    }

    // Named constant (e.g. HIGH_VALUE_THRESHOLD)
    if (right.match(/^[A-Z_]+$/)) {
      const rhsId = camelize(right.replace(/_THRESHOLD$/, '').replace(/_/g, ' ').toLowerCase())
      return isGt
        ? { name: base + capitalize(rhsId) + 'Exceeded',       trueMeaning: `${left} ${eop} ${right}`, falseMeaning: `${left} below ${right}` }
        : { name: base + capitalize(rhsId) + 'BelowThreshold', trueMeaning: `${left} below ${right}`,  falseMeaning: `${left} ${eop} ${right}` }
    }

    // Numeric literal
    const rhsId = right.replace('.', '_')
    return isGt
      ? { name: base + 'Above' + capitalize(rhsId), trueMeaning: `${left} ${eop} ${right}`, falseMeaning: `${left} not above ${right}` }
      : { name: base + 'Below' + capitalize(rhsId), trueMeaning: `${left} ${eop} ${right}`, falseMeaning: `${left} not below ${right}` }
  }

  // Generic fallback
  return fallback(text)
}

function deriveCall(text: string, negated: boolean): Names {
  // e.g. `canTransition(order.status, 'approved')`
  //      `/[A-Z]/.test(password)`
  //      `this.orderRepo.findById(input.orderId)`
  const m = text.match(/^(?:await\s+)?(?:this\.)?(?:[\w.]+\.)?(\w+)\(([^)]*)\)/)
  if (!m) return fallback(text)

  const fnName = m[1]
  const rawArgs = m[2]

  // Extract first string literal argument as context
  const firstStringArg = rawArgs.match(/['"]([^'"]+)['"]/)
  const argSuffix = firstStringArg ? capitalize(camelize(firstStringArg[1])) : ''

  const base = camelize(fnName) + argSuffix  // e.g. canTransitionApproved, testPassword

  if (negated) {
    return {
      name: base + 'Failed',
      trueMeaning: `${fnName}(${rawArgs}) returned false`,
      falseMeaning: `${fnName}(${rawArgs}) returned true`,
    }
  }
  return {
    name: base + 'Passed',
    trueMeaning: `${fnName}(${rawArgs}) returned true`,
    falseMeaning: `${fnName}(${rawArgs}) returned false`,
  }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

// Convert a JS property path to a camelCase identifier
// "operator.role" → "operatorRole"
// "this.orderRepo.findById" → "orderRepoFindById"
// "order.items.length" → "orderItemsLength"
function pathToIdent(text: string): string {
  let s = text.trim()
  s = s.replace(/^await\s+/, '')
  s = s.replace(/^this\./, '')
  // Strip array indexing
  s = s.replace(/\[.*?\]/g, '')
  // Stop at operator or paren
  s = s.split(/[\s!<>=()]/)[0]
  const parts = s.split('.').filter(Boolean)
  if (parts.length === 0) return camelize(s)
  return parts[0] + parts.slice(1).map(capitalize).join('')
}

function negateOp(op: string): string {
  const map: Record<string, string> = { '>': '<=', '>=': '<', '<': '>=', '<=': '>' }
  return map[op] ?? op
}

function stripLeadingNot(text: string): string {
  return text.replace(/^!+/, '').replace(/^\((.+)\)$/, '$1').trim()
}

function camelize(s: string): string {
  return s
    .trim()
    .split(/[\s_-]+/)
    .map((w, i) => i === 0 ? w.toLowerCase() : capitalize(w))
    .join('')
}

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1)
}

function fallback(text: string): Names {
  const slug = text.replace(/[^a-zA-Z0-9]/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '').slice(0, 40)
  return {
    name: slug || 'unknownCondition',
    trueMeaning: text,
    falseMeaning: `not (${text})`,
  }
}
