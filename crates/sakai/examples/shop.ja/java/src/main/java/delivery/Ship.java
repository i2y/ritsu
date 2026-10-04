// A shipment: once the box is packed (through the layer), on the next business day (the shared
// kernel), for an order of ordering (the partner).
package delivery;

import calendars.Tokyo;
import delivery.acl.inventory.Packing;
import shop.delivery.v1.CreateShipmentRequest;
import shop.ordering.v1.Order;

public final class Ship {
  public CreateShipmentRequest ship(Order order, int packed, String day) {
    if (!Packing.mayShip(packed) || order.id().isEmpty()) {
      return null;
    }
    Tokyo.nextBusinessDay(day);
    return new CreateShipmentRequest("honshu", 1);
  }
}
