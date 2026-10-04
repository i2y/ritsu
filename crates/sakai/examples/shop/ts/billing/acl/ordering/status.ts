// Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
// it. The rule billing/rules/billing_need.rule decides it; this module is where the code meets ordering.
import { OrderStatus } from "../../../shop/ordering/v1/order_pb.ts";

export type Handling = "wait" | "bill" | "skip";

export function handling(status: OrderStatus): Handling {
  switch (status) {
    case OrderStatus.RECEIVED:
      return "wait";
    case OrderStatus.PAID:
    case OrderStatus.SHIPPED:
      return "bill";
    default:
      return "skip";
  }
}
