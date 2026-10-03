// Billing reads an order's status outside its anticorruption layer toward ordering.
package billing;

public final class BadOutsideLayer {
  public boolean cancelled(int status) {
    return shop.ordering.v1.OrderStatus.values()[status] == shop.ordering.v1.OrderStatus.ORDER_STATUS_CANCELLED;
  }
}
