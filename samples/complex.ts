// Complex function with deep nesting, callbacks, and local functions
export function handleOrderApproval(
  order: { id: string; amount: number; items: string[]; status: string },
  user: { role: string; limit: number }
): { approved: boolean; reason: string } {
  if (!order) {
    return { approved: false, reason: 'no order' }
  }

  if (order.status === 'cancelled') {
    return { approved: false, reason: 'cancelled' }
  }

  function isVip(role: string): boolean {
    return role === 'vip' || role === 'premium'
  }

  const limit = isVip(user.role) ? user.limit * 2 : user.limit

  if (order.amount > limit) {
    if (order.items.length > 10) {
      return { approved: false, reason: 'over limit and too many items' }
    } else if (order.items.length > 5) {
      return { approved: false, reason: 'over limit' }
    }
  }

  switch (order.status) {
    case 'pending':
      if (order.amount > 1000) {
        return { approved: false, reason: 'needs review' }
      }
      return { approved: true, reason: 'ok' }
    case 'pre-approved':
      return { approved: true, reason: 'pre-approved' }
    default:
      return { approved: false, reason: 'unknown status' }
  }
}

// Callback hell example
export function fetchAndProcess(ids: string[]): Promise<string[]> {
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      const results = ids.map((id) => {
        return fetch(`/api/${id}`)
          .then((res) => {
            return res.json().then((data) => {
              if (data.error) {
                throw new Error(data.error)
              }
              return data.value
            })
          })
          .catch((err) => {
            console.error(err)
            return null
          })
      })
      Promise.all(results).then((values) => {
        resolve(values.filter(Boolean) as string[])
      })
    }, 100)
  })
}

// Nested functions example
export function buildTransformer(config: { prefix: string; suffix: string }) {
  const prefix = config.prefix

  function addPrefix(s: string) {
    const trimmed = s.trim()
    const wrap = (v: string) => `${prefix}${v}`
    return wrap(trimmed)
  }

  const addSuffix = (s: string) => {
    return `${s}${config.suffix}`
  }

  return function transform(input: string[]): string[] {
    return input.map((item) => addSuffix(addPrefix(item)))
  }
}
