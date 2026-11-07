// Delivery's anticorruption layer toward inventory: a box's packing status, read as whether to ship.
// A box short of an item is refused and goes back to ordering (contexts/delivery.ctx maps the values), and a value the
// map does not name is refused too: the code does not guess what an upstream that added a value means.
package delivery.acl.inventory;

import warehouse.v1.PackingStatus;

public final class Packing {
  private Packing() {}

  public static boolean mayShip(int status) {
    if (status < 0 || status >= PackingStatus.values().length) {
      throw new IllegalArgumentException("packing status " + status + " is not one the map names");
    }
    PackingStatus s = PackingStatus.values()[status];
    if (s == PackingStatus.PACKING_STATUS_PACKED) return true;
    if (s == PackingStatus.PACKING_STATUS_WAITING) return false;
    if (s == PackingStatus.PACKING_STATUS_SHORT) throw new IllegalArgumentException("a box short of an item is not shipped");
    throw new IllegalArgumentException("packing status " + status + " is not one the map names");
  }
}
