// A stand-in for what protoc-gen-go writes from proto/warehouse/v1/stock.proto.
// The import linters look at the boundaries between packages, not at what is in them.
package warehousev1

type Stock int32

const (
	Stock_STOCK_SECURED Stock = 1
	Stock_STOCK_SHORT   Stock = 2
)

type PackingStatus int32

const (
	PackingStatus_PACKING_STATUS_WAITING PackingStatus = 1
	PackingStatus_PACKING_STATUS_PACKED  PackingStatus = 2
	PackingStatus_PACKING_STATUS_SHORT   PackingStatus = 3
)

type ReserveResponse struct {
	Sku   string
	Stock Stock
	Id    *string
}
