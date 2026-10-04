// A stand-in for what `chobo build` writes from inventory/inventory.book: the ledger of stock.
package ledger

func Hold(order, sku string, count int) bool {
	return order != "" && sku != "" && count > 0
}
