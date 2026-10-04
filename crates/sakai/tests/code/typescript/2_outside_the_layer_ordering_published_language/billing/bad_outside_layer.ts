// Billing reads an order's status outside its anticorruption layer toward ordering.
import { OrderStatus } from "../shop/ordering/v1/order_pb.ts";

export function cancelled(status: number): boolean {
  return status === OrderStatus.CANCELLED;
}
