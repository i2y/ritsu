// The stock service answers a reservation with what the ledger says.
import { hold } from "./ledger.ts";
import { type ReserveResponse, Stock } from "../warehouse/v1/stock_pb.ts";

export function reserve(order: string, sku: string, quantity: number): ReserveResponse {
  if (hold(order, sku, quantity)) {
    return { sku, stock: Stock.SECURED, id: `${order}/${sku}` };
  }
  return { sku, stock: Stock.SHORT };
}
