// The host of catalog: it reads the tree of a catalog, and the generated code of the module walks it.
// Written in a directory of its own in the module of the package `ritsu gen` writes (go/), with
// `--module example.com/catalog`.
package main

import (
	"fmt"
	"strings"

	"example.com/catalog/glue/catalog"
	"example.com/catalog/units"
)

func main() {
	garden := catalog.Category{
		Name: "Garden",
		Children: []catalog.Category{
			{Name: "Hand Tools", Items: []catalog.Item{{Sku: "sp-1", Name: "Spade", Price: units.USDc(1999)}, {Sku: "tr-2", Name: "Trowel", Price: units.USDc(899)}}},
			{Name: "Seeds", Children: []catalog.Category{{Name: "Bulbs"}}},
			{Name: "Hoses", Items: []catalog.Item{{Sku: "hr-1", Name: "Hose reel", Price: units.USDc(4500)}}},
		},
	}
	for _, p := range catalog.Paths(garden) {
		fmt.Println(p)
	}
	if kept := catalog.Pruned(garden); kept == nil {
		fmt.Println("kept: nothing")
	} else {
		fmt.Println("kept: " + strings.Join(catalog.Names(*kept), ", "))
	}
	fmt.Println("slugs: " + strings.Join(catalog.Names(catalog.Slugged(garden)), ", "))
	if c := catalog.Cheapest(garden); c == nil {
		fmt.Println("cheapest: none")
	} else {
		fmt.Printf("cheapest: %s, %d cents\n", c.Name, c.Price)
	}
}
