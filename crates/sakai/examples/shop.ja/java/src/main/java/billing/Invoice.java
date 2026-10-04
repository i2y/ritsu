// An invoice: whether to bill (through the layer), the shipment it is for (billing is delivery's
// customer), and the day it is due, a business day in Tokyo (the shared kernel).
package billing;

import billing.acl.ordering.Status;
import calendars.Tokyo;
import shop.delivery.v1.CreateShipmentRequest;

public final class Invoice {
  public String due(int orderStatus, CreateShipmentRequest shipment, String day) {
    if (Status.handling(orderStatus) != Status.Handling.BILL || shipment.parcels() < 1) {
      return null;
    }
    return Tokyo.nextBusinessDay(day);
  }
}
