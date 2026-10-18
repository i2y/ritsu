// Package service is the marketplace's checkout, written by hand on the package ritsu gen writes
// from this project (`ritsu gen --root . --target go --module marketplace/gen`, its go/ put at gen/
// in the module marketplace): the rule splits what the buyer paid, and the book records the sale,
// in one call and with no workflow. What the rule returns is the package's type of its unit, units.USDc,
// which is what the book's sale takes: were the rule to count in whole dollars, the compiler would
// stop here.
package service

import (
	"context"

	"marketplace/gen/books/marketplace"
	"marketplace/gen/rules/salesplit"
	"marketplace/gen/units"
)

// Checkout records a sale of paid for shop: the marketplace's fee by the shop's plan, the rest to
// the shop.
func Checkout(ctx context.Context, book *marketplace.Book, order, shop string, plan salesplit.Plan, paid units.USDc) (marketplace.Result, error) {
	split, err := salesplit.SaleSplit(salesplit.Input{Plan: plan, Paid: paid})
	if err != nil {
		return marketplace.Result{}, err
	}
	return book.Sale.Do(ctx, marketplace.SaleArgs{Order: order, Shop: shop, To_shop: split.ToShop, Fee: split.Fee})
}
