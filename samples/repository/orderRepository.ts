import type { Order, OrderStatus } from '../domain/order.ts'

export type OrderSearchParams = {
  userId?: string
  status?: OrderStatus
  fromDate?: Date
  toDate?: Date
  minAmount?: number
  maxAmount?: number
  limit?: number
  offset?: number
}

export interface OrderRepository {
  findById(id: string): Promise<Order | null>
  findByUserId(userId: string): Promise<Order[]>
  search(params: OrderSearchParams): Promise<Order[]>
  save(order: Order): Promise<Order>
  delete(id: string): Promise<void>
}

// In-memory implementation (for POC / testing)
export class InMemoryOrderRepository implements OrderRepository {
  private store = new Map<string, Order>()

  async findById(id: string): Promise<Order | null> {
    return this.store.get(id) ?? null
  }

  async findByUserId(userId: string): Promise<Order[]> {
    return Array.from(this.store.values()).filter((o) => o.userId === userId)
  }

  async search(params: OrderSearchParams): Promise<Order[]> {
    let results = Array.from(this.store.values())

    if (params.userId) {
      results = results.filter((o) => o.userId === params.userId)
    }
    if (params.status) {
      results = results.filter((o) => o.status === params.status)
    }
    if (params.fromDate) {
      results = results.filter((o) => o.createdAt >= params.fromDate!)
    }
    if (params.toDate) {
      results = results.filter((o) => o.createdAt <= params.toDate!)
    }
    if (params.minAmount !== undefined) {
      results = results.filter((o) => o.totalAmount >= params.minAmount!)
    }
    if (params.maxAmount !== undefined) {
      results = results.filter((o) => o.totalAmount <= params.maxAmount!)
    }

    results.sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime())

    const offset = params.offset ?? 0
    const limit = params.limit ?? 50
    return results.slice(offset, offset + limit)
  }

  async save(order: Order): Promise<Order> {
    this.store.set(order.id, order)
    return order
  }

  async delete(id: string): Promise<void> {
    this.store.delete(id)
  }
}
