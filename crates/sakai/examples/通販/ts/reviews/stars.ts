// Reviews keep to themselves: nothing goes between reviews and billing.
export function average(stars: number[]): number {
  return stars.length === 0 ? 0 : stars.reduce((a, b) => a + b, 0) / stars.length;
}
