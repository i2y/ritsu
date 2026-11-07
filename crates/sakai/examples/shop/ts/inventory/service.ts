// The stock service answers a reservation with what the ledger says, and posts a hold when the order ships.
import { hold, post as postHold } from "./ledger.ts";
import { type ReserveResponse, Stock } from "../warehouse/v1/stock_pb.ts";

export function reserve(lineId: string, sku: string, quantity: number): ReserveResponse {
  const r = hold(lineId, sku, quantity);
  if (r.result !== "refused") {
    return { sku, stock: Stock.SECURED, id: lineId };
  }
  if (r.reason === "key_conflict") {
    return { sku, stock: Stock.CONFLICT };
  }
  return { sku, stock: Stock.SHORT };
}

export function post(id: string): void {
  const r = postHold(id);
  if (r.result === "refused") {
    throw new Error(`the hold ${id} cannot be posted: ${r.reason}`);
  }
}
