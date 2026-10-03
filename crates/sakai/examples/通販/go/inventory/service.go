// The stock service answers a reservation with what the ledger says.
package inventory

import (
	"example.com/shop/inventory/ledger"
	warehousev1 "example.com/shop/warehouse/v1"
)

func Reserve(order, sku string, quantity int) *warehousev1.ReserveResponse {
	if ledger.Hold(order, sku, quantity) {
		id := order + "/" + sku
		return &warehousev1.ReserveResponse{Sku: sku, Stock: warehousev1.Stock_STOCK_SECURED, Id: &id}
	}
	return &warehousev1.ReserveResponse{Sku: sku, Stock: warehousev1.Stock_STOCK_SHORT}
}
