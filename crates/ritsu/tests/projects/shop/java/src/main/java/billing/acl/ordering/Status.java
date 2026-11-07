// Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
// it. The rule billing/rules/billing_need.rule decides it; this class is where the code meets ordering.
// A value the rule does not name (0, or one ordering adds later) raises: the rule stops at it (E032), and so does the code.
package billing.acl.ordering;

import shop.ordering.v1.OrderStatus;

public final class Status {
  public enum Handling { WAIT, BILL, SKIP }

  private Status() {}

  public static Handling handling(int status) {
    if (status < 0 || status >= OrderStatus.values().length) {
      throw new IllegalArgumentException("order status " + status + " is not one the rule reads");
    }
    OrderStatus s = OrderStatus.values()[status];
    return switch (s) {
      case ORDER_STATUS_RECEIVED -> Handling.WAIT;
      case ORDER_STATUS_PAID, ORDER_STATUS_SHIPPED -> Handling.BILL;
      case ORDER_STATUS_CANCELLED -> Handling.SKIP;
      default -> throw new IllegalArgumentException("order status " + status + " is not one the rule reads");
    };
  }
}
