import type { User } from '../domain/user.ts'
import type { Order, OrderItem } from '../domain/order.ts'
import { canPurchase } from '../domain/user.ts'
import { calcOrderTotal, calcShippingFee, applyCoupon } from '../domain/order.ts'
import { validateAddress, type ValidationResult } from '../utils/validation.ts'
import type { OrderRepository } from '../repository/orderRepository.ts'
import type { UserRepository } from '../repository/userRepository.ts'

type PlaceOrderInput = {
  userId: string
  items: OrderItem[]
  couponCode?: string
  shippingAddress: {
    postalCode: string
    prefecture: string
    city: string
    street: string
  }
  note?: string
}

type PlaceOrderResult =
  | { ok: true; order: Order }
  | { ok: false; error: string; validationErrors?: string[] }

export class PlaceOrderUseCase {
  constructor(
    private readonly userRepo: UserRepository,
    private readonly orderRepo: OrderRepository,
  ) {}

  async execute(input: PlaceOrderInput): Promise<PlaceOrderResult> {
    const user = await this.userRepo.findById(input.userId)
    if (!user) {
      return { ok: false, error: 'User not found' }
    }
    if (user.status !== 'active') {
      return { ok: false, error: 'User account is not active' }
    }

    if (input.items.length === 0) {
      return { ok: false, error: 'Order must contain at least one item' }
    }
    for (const item of input.items) {
      if (item.quantity <= 0) {
        return { ok: false, error: `Invalid quantity for item: ${item.productId}` }
      }
      if (item.unitPrice < 0) {
        return { ok: false, error: `Invalid price for item: ${item.productId}` }
      }
    }

    const addressValidation: ValidationResult = validateAddress(input.shippingAddress)
    if (!addressValidation.ok) {
      return { ok: false, error: 'Invalid address', validationErrors: addressValidation.errors }
    }

    const subtotal = calcOrderTotal(input.items)
    const { discountAmount, finalAmount } = applyCoupon(subtotal, input.couponCode ?? null)
    const isMember = user.role === 'member' || user.role === 'vip' || user.role === 'admin'
    const shippingFee = calcShippingFee(finalAmount, isMember)
    const totalWithShipping = finalAmount + shippingFee

    if (!canPurchase(user, totalWithShipping)) {
      return { ok: false, error: 'Purchase amount exceeds user limit' }
    }

    const order: Order = {
      id: crypto.randomUUID(),
      userId: user.id,
      items: input.items,
      status: 'pending',
      totalAmount: subtotal,
      discountAmount,
      shippingFee,
      couponCode: input.couponCode ?? null,
      note: input.note ?? null,
      createdAt: new Date(),
      updatedAt: new Date(),
    }

    const saved = await this.orderRepo.save(order)
    return { ok: true, order: saved }
  }
}
