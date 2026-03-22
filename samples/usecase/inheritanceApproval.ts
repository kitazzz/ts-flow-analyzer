type ApprovalInput = {
  orderId: string
  actorId: string
  note?: string
  force?: boolean
}

type ApprovalResult =
  | { ok: true; order: OrderRecord }
  | { ok: false; error: string }

type OrderRecord = {
  id: string
  status: 'pending' | 'approved' | 'cancelled'
  totalAmount: number
  discountRate: number
  customerId: string | null
  manualReviewRequired: boolean
  items: Array<{ sku: string; quantity: number }>
}

type Actor = {
  id: string
  role: 'admin' | 'manager' | 'staff'
  scopes: string[]
  flags: {
    allowGuestApproval: boolean
    canApproveLargeDiscount: boolean
  }
}

type PolicyDecision =
  | { allowed: true; reason: 'ok' | 'manual-override'; auditRequired: boolean }
  | { allowed: false; reason: string; auditRequired: boolean }

type ApprovalAuditRepo = {
  record(entry: { orderId: string; actorId: string; event: string; detail?: string }): Promise<void>
}

type ApprovalNotifier = {
  publish(topic: string, payload: Record<string, unknown>): Promise<void>
}

type OrderRepository = {
  findById(orderId: string): Promise<OrderRecord | null>
  save(order: OrderRecord): Promise<OrderRecord>
}

type UserRepository = {
  findById(userId: string): Promise<Actor | null>
}

abstract class ApprovalWorkflowBase {
  async execute(input: ApprovalInput): Promise<ApprovalResult> {
    const noteLines =
      input.note
        ?.split('\n')
        .map((line) => line.trim())
        .filter(Boolean) ?? []

    if (noteLines.length > 5 && !input.force) {
      return { ok: false, error: 'Too many note lines for non-forced approval' }
    }

    const order = await this.loadOrder(input.orderId)
    if (!order) {
      return { ok: false, error: 'Order not found' }
    }
    if (order.status !== 'pending') {
      return { ok: false, error: `Order is not pending: ${order.status}` }
    }

    const actor = await this.loadActor(input.actorId)
    if (!actor) {
      return { ok: false, error: 'Actor not found' }
    }
    if (!this.canApprove(actor, order)) {
      return { ok: false, error: 'Actor cannot approve this order' }
    }

    const policy = this.resolvePolicy(order, actor, noteLines, input.force ?? false)
    if (!policy.allowed) {
      return { ok: false, error: `Policy rejected approval: ${policy.reason}` }
    }

    try {
      const prepared = this.prepareApproval(order, input, noteLines, policy.auditRequired)
      const saved = await this.persistApproval(prepared)
      await this.afterPersist(saved, actor, noteLines, policy.auditRequired)
      return { ok: true, order: saved }
    } catch (error) {
      this.onFailure(error, input)
      return { ok: false, error: 'Unexpected approval failure' }
    }
  }

  protected canApprove(actor: Actor, order: OrderRecord): boolean {
    return actor.role === 'admin' || (actor.scopes.includes('approve:order') && !order.manualReviewRequired)
  }

  protected prepareApproval(
    order: OrderRecord,
    input: ApprovalInput,
    noteLines: string[],
    auditRequired: boolean,
  ): OrderRecord {
    const shouldEscalate = auditRequired || (input.force === true && order.totalAmount > 100000)
    if (shouldEscalate) {
      return {
        ...order,
        manualReviewRequired: true,
        status: 'approved',
      }
    }

    if (noteLines.some((line) => line.toLowerCase().includes('cancel'))) {
      throw new Error('Approval note contains cancellation instruction')
    }

    return {
      ...order,
      manualReviewRequired: false,
      status: 'approved',
    }
  }

  protected onFailure(error: unknown, input: ApprovalInput): void {
    console.error('approval failed', { error, input })
  }

  protected async afterPersist(
    _saved: OrderRecord,
    _actor: Actor,
    _noteLines: string[],
    _auditRequired: boolean,
  ): Promise<void> {}

  protected abstract loadOrder(orderId: string): Promise<OrderRecord | null>
  protected abstract loadActor(actorId: string): Promise<Actor | null>
  protected abstract resolvePolicy(
    order: OrderRecord,
    actor: Actor,
    noteLines: string[],
    force: boolean,
  ): PolicyDecision
  protected abstract persistApproval(order: OrderRecord): Promise<OrderRecord>
}

export class EnterpriseApprovalWorkflow extends ApprovalWorkflowBase {
  constructor(
    private readonly orderRepo: OrderRepository,
    private readonly userRepo: UserRepository,
    private readonly auditRepo: ApprovalAuditRepo,
    private readonly notifier: ApprovalNotifier,
  ) {
    super()
  }

  protected loadOrder(orderId: string): Promise<OrderRecord | null> {
    return this.orderRepo.findById(orderId)
  }

  protected loadActor(actorId: string): Promise<Actor | null> {
    return this.userRepo.findById(actorId)
  }

  protected resolvePolicy(
    order: OrderRecord,
    actor: Actor,
    noteLines: string[],
    force: boolean,
  ): PolicyDecision {
    const hasLargeDiscount = order.discountRate >= 0.4
    const hasForbiddenSku = order.items.some((item) => item.sku.startsWith('X-'))
    const mentionsManualOverride = noteLines.some((line) => line.includes('manual override'))

    if (!order.customerId && !actor.flags.allowGuestApproval) {
      return { allowed: false, reason: 'guest-order-not-allowed', auditRequired: true }
    }

    if ((hasLargeDiscount && !actor.flags.canApproveLargeDiscount) || hasForbiddenSku) {
      return { allowed: false, reason: 'risk-rule-triggered', auditRequired: true }
    }

    if (force && actor.role !== 'admin') {
      return { allowed: false, reason: 'force-requires-admin', auditRequired: true }
    }

    return {
      allowed: true,
      reason: mentionsManualOverride ? 'manual-override' : 'ok',
      auditRequired: mentionsManualOverride || order.totalAmount >= 50000,
    }
  }

  protected async persistApproval(order: OrderRecord): Promise<OrderRecord> {
    return this.orderRepo.save(order)
  }

  protected override async afterPersist(
    saved: OrderRecord,
    actor: Actor,
    noteLines: string[],
    auditRequired: boolean,
  ): Promise<void> {
    await super.afterPersist(saved, actor, noteLines, auditRequired)

    if (auditRequired) {
      await this.auditRepo.record({
        orderId: saved.id,
        actorId: actor.id,
        event: 'approval.audit-required',
        detail: noteLines.join(' | '),
      })
    }

    if (saved.totalAmount >= 200000 && actor.role !== 'admin') {
      throw new Error('High-value approvals must be reviewed by admin')
    }

    await this.notifier.publish('order.approved', {
      orderId: saved.id,
      actorId: actor.id,
      lineCount: noteLines.length,
    })
  }
}
