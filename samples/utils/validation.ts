export type ValidationResult = { ok: true } | { ok: false; errors: string[] }

export function validateEmail(email: string): ValidationResult {
  const errors: string[] = []
  if (!email || email.trim() === '') {
    errors.push('Email is required')
    return { ok: false, errors }
  }
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    errors.push('Email format is invalid')
  }
  if (email.length > 254) {
    errors.push('Email must be 254 characters or fewer')
  }
  return errors.length > 0 ? { ok: false, errors } : { ok: true }
}

export function validatePassword(password: string): ValidationResult {
  const errors: string[] = []
  if (!password) {
    return { ok: false, errors: ['Password is required'] }
  }
  if (password.length < 8) {
    errors.push('Password must be at least 8 characters')
  }
  if (!/[A-Z]/.test(password)) {
    errors.push('Password must contain at least one uppercase letter')
  }
  if (!/[a-z]/.test(password)) {
    errors.push('Password must contain at least one lowercase letter')
  }
  if (!/[0-9]/.test(password)) {
    errors.push('Password must contain at least one digit')
  }
  return errors.length > 0 ? { ok: false, errors } : { ok: true }
}

export function validateOrderAmount(amount: number, userLimit: number, isVip: boolean): ValidationResult {
  const errors: string[] = []
  const effectiveLimit = isVip ? userLimit * 2 : userLimit

  if (amount <= 0) {
    errors.push('Amount must be positive')
  }
  if (amount > effectiveLimit) {
    errors.push(`Amount ${amount} exceeds limit ${effectiveLimit}`)
  }
  if (amount > 1_000_000) {
    errors.push('Single order cannot exceed 1,000,000')
  }
  return errors.length > 0 ? { ok: false, errors } : { ok: true }
}

export function validateAddress(address: {
  postalCode: string
  prefecture: string
  city: string
  street: string
}): ValidationResult {
  const errors: string[] = []

  if (!/^\d{3}-?\d{4}$/.test(address.postalCode)) {
    errors.push('Invalid postal code format')
  }
  if (!address.prefecture || address.prefecture.trim() === '') {
    errors.push('Prefecture is required')
  }
  if (!address.city || address.city.trim() === '') {
    errors.push('City is required')
  }
  if (!address.street || address.street.trim() === '') {
    errors.push('Street is required')
  }
  if (address.street && address.street.length > 200) {
    errors.push('Street address is too long')
  }
  return errors.length > 0 ? { ok: false, errors } : { ok: true }
}
