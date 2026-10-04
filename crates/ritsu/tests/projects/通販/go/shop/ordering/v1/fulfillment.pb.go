// A stand-in for what protoc-gen-go writes from proto/shop/ordering/v1/fulfillment.proto.
// The proto imports warehouse/v1/stock.proto, so the package imports the one written from that.
package orderingv1

import warehousev1 "example.com/shop/warehouse/v1"

type FulfillResponse struct {
	Reservations   []*warehousev1.ReserveResponse
	TrackingNumber string
}
