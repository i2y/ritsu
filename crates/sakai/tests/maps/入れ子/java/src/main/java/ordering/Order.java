// Ordering conforms to inventory and takes its published language.
package ordering;

import warehouse.v1.Stock;

public final class Order {
  public int count(Stock s) {
    return s.count();
  }
}
