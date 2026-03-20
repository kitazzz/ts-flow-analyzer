export function truncate(str: string, maxLength: number, suffix = '...'): string {
  if (str.length <= maxLength) return str
  return str.slice(0, maxLength - suffix.length) + suffix
}

export function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .replace(/[\s_-]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

export function camelToSnake(str: string): string {
  return str.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`)
}

export function snakeToCamel(str: string): string {
  return str.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())
}

export function formatCurrency(amount: number, currency = 'JPY', locale = 'ja-JP'): string {
  return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(amount)
}

export function maskEmail(email: string): string {
  const [local, domain] = email.split('@')
  if (!local || !domain) return email
  const visible = local.length > 2 ? local.slice(0, 2) : local[0] ?? ''
  return `${visible}${'*'.repeat(Math.max(local.length - 2, 1))}@${domain}`
}

export function parseQueryString(query: string): Record<string, string> {
  const result: Record<string, string> = {}
  if (!query || query === '?') return result
  const normalized = query.startsWith('?') ? query.slice(1) : query
  for (const pair of normalized.split('&')) {
    const [key, value] = pair.split('=')
    if (key) {
      result[decodeURIComponent(key)] = decodeURIComponent(value ?? '')
    }
  }
  return result
}
