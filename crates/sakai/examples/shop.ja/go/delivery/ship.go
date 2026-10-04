// A shipment: once the box is packed (through the layer), on the next business day (the shared
// kernel), for an order of ordering (the partner).
package delivery

import (
	"example.com/shop/calendars"
	aclinventory "example.com/shop/delivery/acl/inventory"
	deliveryv1 "example.com/shop/shop/delivery/v1"
	orderingv1 "example.com/shop/shop/ordering/v1"
)

func Ship(order *orderingv1.Order, packed int32, day string) *deliveryv1.CreateShipmentRequest {
	ok, err := aclinventory.MayShip(packed)
	if err != nil || !ok || order.Id == "" {
		return nil
	}
	calendars.NextBusinessDay(day)
	return &deliveryv1.CreateShipmentRequest{Region: "honshu", Parcels: 1}
}
