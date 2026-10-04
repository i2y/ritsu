// A stand-in for what protoc-gen-go writes from proto/shop/delivery/v1/shipment.proto.
package deliveryv1

type Handling int32

const (
	Handling_HANDLING_STANDARD Handling = 0
	Handling_HANDLING_FRAGILE  Handling = 1
)

type CreateShipmentRequest struct {
	Region  string
	Parcels int32
}
