// The host of stock_diff: it reads two counts of the stockroom and the moves of a day, and the
// generated code of the module compares them. A dict and a set are maps, which Go goes round in no
// order, so the host sorts their keys. Written in a directory of its own in the module of the
// package `ritsu gen` writes (go/), with `--module example.com/stock_diff`.
package main

import (
	"fmt"
	"maps"
	"slices"
	"strings"

	stock "example.com/stock_diff/glue/stockdiff"
)

func main() {
	before := map[string]int64{"ink": 4, "pen": 10, "pad": 2}
	after := map[string]int64{"pen": 7, "pad": 2, "tape": 3}
	for _, c := range stock.Changes(before, after) {
		switch c := c.(type) {
		case stock.ChangeAdded:
			fmt.Printf("added %s: %d\n", c.Sku, c.Qty)
		case stock.ChangeRemoved:
			fmt.Printf("removed %s: %d\n", c.Sku, c.Qty)
		case stock.ChangeChanged:
			fmt.Printf("changed %s: %d -> %d\n", c.Sku, c.Was, c.Now)
		}
	}
	today := stock.Moved(after, []stock.Move{{Sku: "pen", Qty: -7}, {Sku: "glue", Qty: 12}})
	var counts []string
	for _, sku := range slices.Sorted(maps.Keys(today)) {
		counts = append(counts, fmt.Sprintf("%s %d", sku, today[sku]))
	}
	fmt.Println("after the day: " + strings.Join(counts, ", "))
	fmt.Println("to reorder: " + strings.Join(slices.Sorted(maps.Keys(stock.ToReorder(today, 5))), ", "))
}
