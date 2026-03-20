import type { Order } from '../domain/order.ts'
import { canTransition, transitionOrder } from '../domain/order.ts'
import type { OrderRepository } from '../repository/orderRepository.ts'
import type { UserRepository } from '../repository/userRepository.ts'

type CancelOrderInput = {
  orderId: string
  requesterId: string
  reason: string
}

type CancelOrderResult =
  | { ok: true; order: Order; refundAmount: number }
  | { ok: false; error: string }

const CANCEL_FEE_RATE = 0.05   // 5% cancellation fee after approval
const NO_FEE_STATUSES = new Set(['draft', 'pending'])

export class CancelOrderUseCase {
  constructor(
    private readonly orderRepo: OrderRepository,
    private readonly userRepo: UserRepository,
  ) {}

  async execute(input: CancelOrderInput): Promise<CancelOrderResult> {
    if (!input.reason || input.reason.trim() === '') {
      return { ok: false, error: 'Cancellation reason is required' }
    }

    const order = await this.orderRepo.findById(input.orderId)
    if (!order) {
      return { ok: false, error: 'Order not found' }
    }

    const requester = await this.userRepo.findById(input.requesterId)
    if (!requester) {
      return { ok: false, error: 'Requester not found' }
    }

    const isOwner = order.userId === input.requesterId
    const isAdmin = requester.role === 'admin'

    if (!isOwner && !isAdmin) {
      return { ok: false, error: 'Not authorized to cancel this order' }
    }

    if (!canTransition(order.status, 'cancelled')) {
      return { ok: false, error: `Order in status '${order.status}' cannot be cancelled` }
    }

    const refundAmount = this.calcRefund(order)
    const cancelled = transitionOrder(order, 'cancelled')
    const saved = await this.orderRepo.save({
      ...cancelled,
      note: [order.note, `Cancelled: ${input.reason}`].filter(Boolean).join(' / '),
    })

    return { ok: true, order: saved, refundAmount }
  }

  private calcRefund(order: Order): number {
    const paid = order.totalAmount - order.discountAmount + order.shippingFee

    if (NO_FEE_STATUSES.has(order.status)) {
      return paid
    }

    // Post-approval: charge cancellation fee
    const fee = Math.floor(paid * CANCEL_FEE_RATE)
    return paid - fee
  }
}
