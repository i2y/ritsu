// Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
// it. The rule billing/rules/請求の要否.rule decides it; this package is where the code meets ordering.
package ordering

import orderingv1 "example.com/shop/shop/ordering/v1"

type Handling string

const (
	Wait Handling = "wait"
	Bill Handling = "bill"
	Skip Handling = "skip"
)

func HandlingOf(status int32) Handling {
	switch orderingv1.OrderStatus(status) {
	case orderingv1.OrderStatus_ORDER_STATUS_RECEIVED:
		return Wait
	case orderingv1.OrderStatus_ORDER_STATUS_PAID, orderingv1.OrderStatus_ORDER_STATUS_SHIPPED:
		return Bill
	default:
		return Skip
	}
}
