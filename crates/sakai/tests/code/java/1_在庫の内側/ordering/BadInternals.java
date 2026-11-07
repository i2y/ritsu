// Ordering takes stock from inventory's ledger, the inside of inventory.
package ordering;

public final class BadInternals {
  public boolean take(String order, String sku) {
    var held = inventory.Ledger.hold(order, sku, 1);
    return held != null;
  }
}
