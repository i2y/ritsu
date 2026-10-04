// An invoice: whether to bill (through the layer), the shipment it is for (billing is delivery's
// customer), and the day it is due, a business day in Tokyo (the shared kernel).
package billing

import (
	aclordering "example.com/shop/billing/acl/ordering"
	"example.com/shop/calendars"
	deliveryv1 "example.com/shop/shop/delivery/v1"
)

func Due(orderStatus int32, shipment *deliveryv1.CreateShipmentRequest, day string) (string, bool) {
	if aclordering.HandlingOf(orderStatus) != aclordering.Bill || shipment.Parcels < 1 {
		return "", false
	}
	return calendars.NextBusinessDay(day), true
}
