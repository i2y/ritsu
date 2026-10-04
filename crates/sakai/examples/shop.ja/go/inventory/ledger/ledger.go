// A stand-in for what `chobo build` writes from inventory/在庫の引当.book: the ledger of stock.
package ledger

func Hold(order, sku string, count int) bool {
	return order != "" && sku != "" && count > 0
}
