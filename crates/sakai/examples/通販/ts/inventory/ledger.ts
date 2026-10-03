// A stand-in for the module `chobo build` writes from inventory/在庫の引当.book: the ledger of stock.
export function hold(order: string, sku: string, count: number): boolean {
  return order !== "" && sku !== "" && count > 0;
}
