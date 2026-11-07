// Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
// it. The rule billing/rules/請求の要否.rule decides it; this package is where the code meets ordering.
// A value the rule does not name (0, or one ordering adds later) raises: the rule stops at it (E032), and so does the code.
package ordering

import (
	"fmt"

	orderingv1 "example.com/shop/shop/ordering/v1"
)

type Handling string

const (
	Wait Handling = "wait"
	Bill Handling = "bill"
	Skip Handling = "skip"
)

func HandlingOf(status int32) (Handling, error) {
	switch orderingv1.OrderStatus(status) {
	case orderingv1.OrderStatus_ORDER_STATUS_RECEIVED:
		return Wait, nil
	case orderingv1.OrderStatus_ORDER_STATUS_PAID, orderingv1.OrderStatus_ORDER_STATUS_SHIPPED:
		return Bill, nil
	case orderingv1.OrderStatus_ORDER_STATUS_CANCELLED:
		return Skip, nil
	default:
		return "", fmt.Errorf("order status %d is not one the rule reads", status)
	}
}
