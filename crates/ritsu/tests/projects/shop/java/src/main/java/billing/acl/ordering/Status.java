// Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
// it. The rule billing/rules/billing_need.rule decides it; this class is where the code meets ordering.
package billing.acl.ordering;

import shop.ordering.v1.OrderStatus;

public final class Status {
  public enum Handling { WAIT, BILL, SKIP }

  private Status() {}

  public static Handling handling(int status) {
    OrderStatus s = OrderStatus.values()[status];
    return switch (s) {
      case ORDER_STATUS_RECEIVED -> Handling.WAIT;
      case ORDER_STATUS_PAID, ORDER_STATUS_SHIPPED -> Handling.BILL;
      default -> Handling.SKIP;
    };
  }
}
