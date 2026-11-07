// Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
// it. The rule billing/rules/請求の要否.rule decides it; this module is where the code meets ordering.
// A value the rule does not name (0, or one ordering adds later) raises: the rule stops at it (E032), and so does the code.
import { OrderStatus } from "../../../shop/ordering/v1/order_pb.ts";

export type Handling = "wait" | "bill" | "skip";

export function handling(status: OrderStatus): Handling {
  switch (status) {
    case OrderStatus.RECEIVED:
      return "wait";
    case OrderStatus.PAID:
    case OrderStatus.SHIPPED:
      return "bill";
    case OrderStatus.CANCELLED:
      return "skip";
    default:
      throw new Error(`order status ${status} is not one the rule reads`);
  }
}
