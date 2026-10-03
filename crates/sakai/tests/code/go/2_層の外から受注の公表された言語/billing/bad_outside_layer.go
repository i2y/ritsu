// Billing reads an order's status outside its anticorruption layer toward ordering.
package billing

import orderingv1 "example.com/shop/shop/ordering/v1"

func Cancelled(status int32) bool {
	return orderingv1.OrderStatus(status) == orderingv1.OrderStatus_ORDER_STATUS_CANCELLED
}
