// Ordering conforms to inventory: it takes the warehouse's types as they are.
package ordering

import (
	orderingv1 "example.com/shop/shop/ordering/v1"
	warehousev1 "example.com/shop/warehouse/v1"
)

func Short(answers []*warehousev1.ReserveResponse) bool {
	for _, a := range answers {
		if a.Stock == warehousev1.Stock_STOCK_SHORT {
			return true
		}
	}
	return false
}

func Received(id string, amountJpy int64) *orderingv1.Order {
	return &orderingv1.Order{Id: id, Status: orderingv1.OrderStatus_ORDER_STATUS_RECEIVED, AmountJpy: amountJpy}
}
