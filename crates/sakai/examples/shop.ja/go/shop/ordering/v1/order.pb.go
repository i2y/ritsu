// A stand-in for what protoc-gen-go writes from proto/shop/ordering/v1/order.proto.
package orderingv1

type OrderStatus int32

const (
	OrderStatus_ORDER_STATUS_RECEIVED  OrderStatus = 1
	OrderStatus_ORDER_STATUS_PAID      OrderStatus = 2
	OrderStatus_ORDER_STATUS_SHIPPED   OrderStatus = 3
	OrderStatus_ORDER_STATUS_CANCELLED OrderStatus = 4
)

type Order struct {
	Id        string
	Status    OrderStatus
	AmountJpy int64
}
