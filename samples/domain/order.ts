export type OrderStatus =
  | 'draft'
  | 'pending'
  | 'approved'
  | 'shipped'
  | 'delivered'
  | 'cancelled'
  | 'refunded'

export type OrderItem = {
  productId: string
  name: string
  unitPrice: number
  quantity: number
}

export type Order = {
  id: string
  userId: string
  items: OrderItem[]
  status: OrderStatus
  totalAmount: number
  discountAmount: number
  shippingFee: number
  couponCode: string | null
  note: string | null
  createdAt: Date
  updatedAt: Date
}

export function calcOrderTotal(items: OrderItem[]): number {
  return items.reduce((sum, item) => sum + item.unitPrice * item.quantity, 0)
}

export function calcShippingFee(totalAmount: number, isMember: boolean): number {
  if (isMember && totalAmount >= 5_000) return 0
  if (totalAmount >= 10_000) return 0
  if (totalAmount >= 3_000) return 300
  return 600
}

export function applyCoupon(
  totalAmount: number,
  couponCode: string | null,
): { discountAmount: number; finalAmount: number } {
  if (!couponCode) return { discountAmount: 0, finalAmount: totalAmount }

  if (couponCode.startsWith('FLAT')) {
    const amount = parseInt(couponCode.replace('FLAT', ''), 10)
    if (!isNaN(amount) && amount > 0) {
      const discountAmount = Math.min(amount, totalAmount)
      return { discountAmount, finalAmount: totalAmount - discountAmount }
    }
  }

  if (couponCode.startsWith('PCT')) {
    const pct = parseInt(couponCode.replace('PCT', ''), 10)
    if (!isNaN(pct) && pct > 0 && pct <= 100) {
      const discountAmount = Math.floor(totalAmount * (pct / 100))
      return { discountAmount, finalAmount: totalAmount - discountAmount }
    }
  }

  return { discountAmount: 0, finalAmount: totalAmount }
}

export function canTransition(from: OrderStatus, to: OrderStatus): boolean {
  const allowed: Record<OrderStatus, OrderStatus[]> = {
    draft: ['pending', 'cancelled'],
    pending: ['approved', 'cancelled'],
    approved: ['shipped', 'cancelled'],
    shipped: ['delivered', 'refunded'],
    delivered: ['refunded'],
    cancelled: [],
    refunded: [],
  }
  return allowed[from]?.includes(to) ?? false
}

export function transitionOrder(order: Order, to: OrderStatus): Order {
  if (!canTransition(order.status, to)) {
    throw new Error(`Invalid transition: ${order.status} → ${to}`)
  }
  return { ...order, status: to, updatedAt: new Date() }
}

export function summarizeOrder(order: Order): string {
  const lines = [
    `Order ${order.id} [${order.status}]`,
    `  Items: ${order.items.length}`,
    `  Subtotal: ¥${order.totalAmount.toLocaleString()}`,
  ]
  if (order.discountAmount > 0) {
    lines.push(`  Discount: -¥${order.discountAmount.toLocaleString()}`)
  }
  if (order.shippingFee > 0) {
    lines.push(`  Shipping: ¥${order.shippingFee.toLocaleString()}`)
  }
  const total = order.totalAmount - order.discountAmount + order.shippingFee
  lines.push(`  Total: ¥${total.toLocaleString()}`)
  if (order.note) {
    lines.push(`  Note: ${order.note}`)
  }
  return lines.join('\n')
}
