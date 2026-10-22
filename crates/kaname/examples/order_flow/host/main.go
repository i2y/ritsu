// The host of order_flow: it keeps the order and takes the events, and the generated code of the
// module says what each event makes of the order. Written in a directory of its own in the module
// of the package `ritsu gen` writes (go/), with `--module example.com/order_flow`.
package main

import (
	"fmt"

	"example.com/order_flow/glue/kaname"
	flow "example.com/order_flow/glue/orderflow"
	"example.com/order_flow/units"
)

// name is the name of a value, as the module writes it.
func name(v any) string {
	switch v.(type) {
	case flow.EventAdd:
		return "add"
	case flow.EventPlace:
		return "place"
	case flow.EventPay:
		return "pay"
	case flow.EventShip:
		return "ship"
	case flow.EventCancel:
		return "cancel"
	case flow.OrderCart:
		return "cart"
	case flow.OrderPlaced:
		return "placed"
	case flow.OrderPaid:
		return "paid"
	case flow.OrderShipped:
		return "shipped"
	case flow.OrderCancelled:
		return "cancelled"
	}
	return "?"
}

func main() {
	events := []flow.Event{
		flow.EventAdd{Line: flow.Line{Sku: "pen", Qty: 2, Price: units.USDc(150)}},
		flow.EventAdd{Line: flow.Line{Sku: "ink", Qty: 1, Price: units.USDc(400)}},
		flow.EventPlace{On: kaname.Date{Year: 2026, Month: 10, Day: 22}},
		flow.EventPay{Amount: units.USDc(650)},
		flow.EventPay{Amount: units.USDc(700)},
		flow.EventShip{Tracking: "1Z-42"},
		flow.EventCancel{Reason: "too late"},
	}
	var order flow.Order = flow.OrderCart{}
	for _, event := range events {
		switch r := flow.Next(order, event).(type) {
		case kaname.Err[flow.Order, string]:
			fmt.Printf("%s: refused: %s\n", name(event), r.Error)
		case kaname.Ok[flow.Order, string]:
			order = r.Value
			fmt.Printf("%s: %s\n", name(event), name(order))
		}
	}
}
