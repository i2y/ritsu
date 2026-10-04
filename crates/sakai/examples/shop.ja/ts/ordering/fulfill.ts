// Ordering conforms to inventory: it takes the warehouse's types as they are.
import { type Order, OrderStatus } from "../shop/ordering/v1/order_pb.ts";
import { type ReserveResponse, Stock } from "../warehouse/v1/stock_pb.ts";

export function short(answers: ReserveResponse[]): boolean {
  return answers.some((a) => a.stock === Stock.SHORT);
}

export function received(id: string, amountJpy: bigint): Order {
  return { id, status: OrderStatus.RECEIVED, amountJpy };
}
