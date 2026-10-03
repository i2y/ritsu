// Delivery's anticorruption layer toward inventory: a box's packing status, read as whether to ship.
// A box short of an item is refused and goes back to ordering (contexts/配送.ctx maps the values).
package delivery.acl.inventory;

import warehouse.v1.PackingStatus;

public final class Packing {
  private Packing() {}

  public static boolean mayShip(int status) {
    PackingStatus s = PackingStatus.values()[status];
    if (s == PackingStatus.PACKING_STATUS_PACKED) return true;
    if (s == PackingStatus.PACKING_STATUS_WAITING) return false;
    throw new IllegalArgumentException("a box short of an item is not shipped");
  }
}
