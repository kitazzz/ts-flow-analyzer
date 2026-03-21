export type UserRole = 'guest' | 'member' | 'vip' | 'admin'

export type UserStatus = 'active' | 'suspended' | 'withdrawn'

export type User = {
  id: string
  email: string
  name: string
  role: UserRole
  status: UserStatus
  purchaseLimit: number
  totalSpent: number
  createdAt: Date
}

export function createUser(params: {
  id: string
  email: string
  name: string
}): User {
  return {
    ...params,
    role: 'member',
    status: 'active',
    purchaseLimit: 100_000,
    totalSpent: 0,
    createdAt: new Date(),
  }
}

export function promoteToVip(user: User): User {
  if (user.status !== 'active') {
    throw new Error(`Cannot promote non-active user: ${user.id}`)
  }
  if (user.role === 'admin') {
    throw new Error('Admin users cannot be VIP')
  }
  return { ...user, role: 'vip', purchaseLimit: 500_000 }
}

export function suspendUser(user: User, reason: string): User {
  if (user.status === 'withdrawn') {
    throw new Error('Cannot suspend withdrawn user')
  }
  if (user.role === 'admin') {
    throw new Error('Cannot suspend admin')
  }
  console.warn(`User ${user.id} suspended: ${reason}`)
  return { ...user, status: 'suspended' }
}

export function canPurchase(user: User, amount: number): boolean {
  if (user.status !== 'active') return false
  if (user.role === 'guest' && amount > 10_000) return false
  const effectiveLimit = user.role === 'vip' ? user.purchaseLimit * 2 : user.purchaseLimit
  return amount <= effectiveLimit
}

export function effectivePurchaseLimit(user: User): number {
  switch (user.role) {
    case 'vip':
      return user.purchaseLimit * 2
    case 'admin':
      return Infinity
    case 'guest':
      return 10_000
    default:
      return user.purchaseLimit
  }
}
