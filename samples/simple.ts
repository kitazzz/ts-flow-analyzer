// Simple function — should produce CC=1
export function add(a: number, b: number): number {
  return a + b
}

// Moderate complexity
export function classify(score: number): string {
  if (score >= 90) {
    return 'A'
  } else if (score >= 80) {
    return 'B'
  } else if (score >= 70) {
    return 'C'
  } else {
    return 'F'
  }
}

// Switch + ternary
export function getLabel(status: string): string {
  switch (status) {
    case 'active':
      return 'Active'
    case 'inactive':
      return 'Inactive'
    case 'pending':
      return 'Pending'
    default:
      return 'Unknown'
  }
}

// Arrow function assigned to variable
export const computeDiscount = (price: number, vip: boolean): number => {
  return vip ? price * 0.8 : price
}
