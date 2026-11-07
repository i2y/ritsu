// Ordering takes stock from inventory's ledger, the inside of inventory.
package ordering

import "example.com/shop/inventory/ledger"

func Take(order, sku string) bool {
	result, _ := ledger.Hold(order, sku, 1)
	return result == "done"
}
