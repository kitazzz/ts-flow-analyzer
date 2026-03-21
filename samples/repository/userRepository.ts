import type { User, UserRole, UserStatus } from '../domain/user.ts'

export type UserSearchParams = {
  role?: UserRole
  status?: UserStatus
  email?: string
  limit?: number
  offset?: number
}

export interface UserRepository {
  findById(id: string): Promise<User | null>
  findByEmail(email: string): Promise<User | null>
  search(params: UserSearchParams): Promise<User[]>
  save(user: User): Promise<User>
}

export class InMemoryUserRepository implements UserRepository {
  private store = new Map<string, User>()

  async findById(id: string): Promise<User | null> {
    return this.store.get(id) ?? null
  }

  async findByEmail(email: string): Promise<User | null> {
    for (const user of this.store.values()) {
      if (user.email === email) return user
    }
    return null
  }

  async search(params: UserSearchParams): Promise<User[]> {
    let results = Array.from(this.store.values())

    if (params.role) {
      results = results.filter((u) => u.role === params.role)
    }
    if (params.status) {
      results = results.filter((u) => u.status === params.status)
    }
    if (params.email) {
      const q = params.email.toLowerCase()
      results = results.filter((u) => u.email.toLowerCase().includes(q))
    }

    const offset = params.offset ?? 0
    const limit = params.limit ?? 50
    return results.slice(offset, offset + limit)
  }

  async save(user: User): Promise<User> {
    this.store.set(user.id, user)
    return user
  }
}
