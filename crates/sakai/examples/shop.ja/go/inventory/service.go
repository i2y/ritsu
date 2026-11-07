// The stock service answers a reservation with what the ledger says, and posts a hold when the order ships.
package inventory

import (
	"fmt"

	"example.com/shop/inventory/ledger"
	warehousev1 "example.com/shop/warehouse/v1"
)

func Reserve(lineID, sku string, quantity int) *warehousev1.ReserveResponse {
	result, reason := ledger.Hold(lineID, sku, quantity)
	if result != "refused" {
		return &warehousev1.ReserveResponse{Sku: sku, Stock: warehousev1.Stock_STOCK_SECURED, Id: &lineID}
	}
	if reason == "key_conflict" {
		return &warehousev1.ReserveResponse{Sku: sku, Stock: warehousev1.Stock_STOCK_CONFLICT}
	}
	return &warehousev1.ReserveResponse{Sku: sku, Stock: warehousev1.Stock_STOCK_SHORT}
}

func Post(id string) error {
	if result, reason := ledger.Post(id); result == "refused" {
		return fmt.Errorf("the hold %s cannot be posted: %s", id, reason)
	}
	return nil
}
