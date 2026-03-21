import type { Order } from '../domain/order.ts'
import type { User } from '../domain/user.ts'
import { canTransition, transitionOrder } from '../domain/order.ts'
import type { OrderRepository } from '../repository/orderRepository.ts'
import type { UserRepository } from '../repository/userRepository.ts'

type ApproveOrderInput = {
  orderId: string
  operatorId: string
  forceApprove?: boolean
}

type ApproveOrderResult =
  | { ok: true; order: Order }
  | { ok: false; error: string }

const HIGH_VALUE_THRESHOLD = 100_000
const BULK_ITEM_THRESHOLD = 20

export class ApproveOrderUseCase {
  constructor(
    private readonly orderRepo: OrderRepository,
    private readonly userRepo: UserRepository,
  ) {}

  async execute(input: ApproveOrderInput): Promise<ApproveOrderResult> {
    const order = await this.orderRepo.findById(input.orderId)
    if (!order) {
      return { ok: false, error: 'Order not found' }
    }

    const operator = await this.userRepo.findById(input.operatorId)
    if (!operator) {
      return { ok: false, error: 'Operator not found' }
    }
    if (operator.role !== 'admin' && operator.role !== 'vip') {
      return { ok: false, error: 'Insufficient permission to approve orders' }
    }

    if (!canTransition(order.status, 'approved')) {
      return { ok: false, error: `Order cannot be approved from status: ${order.status}` }
    }

    if (!input.forceApprove) {
      const riskCheck = this.checkRisk(order)
      if (!riskCheck.ok) {
        return { ok: false, error: `Risk check failed: ${riskCheck.reason}` }
      }
    }

    const approved = transitionOrder(order, 'approved')
    const saved = await this.orderRepo.save(approved)
    return { ok: true, order: saved }
  }

  private checkRisk(order: Order): { ok: true } | { ok: false; reason: string } {
    const total = order.totalAmount - order.discountAmount + order.shippingFee

    if (total >= HIGH_VALUE_THRESHOLD) {
      return { ok: false, reason: `High value order: ¥${total.toLocaleString()}` }
    }

    const totalItems = order.items.reduce((sum, item) => sum + item.quantity, 0)
    if (totalItems >= BULK_ITEM_THRESHOLD) {
      return { ok: false, reason: `Bulk order: ${totalItems} items` }
    }

    const hasZeroPriceItem = order.items.some((item) => item.unitPrice === 0)
    if (hasZeroPriceItem) {
      return { ok: false, reason: 'Order contains zero-price item' }
    }

    return { ok: true }
  }
}
