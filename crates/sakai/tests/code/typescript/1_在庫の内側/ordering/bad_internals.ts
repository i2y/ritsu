// Ordering takes stock from inventory's ledger, the inside of inventory.
import { hold } from "../inventory/ledger.ts";

export function take(order: string, sku: string): boolean {
  return hold(order, sku, 1);
}
