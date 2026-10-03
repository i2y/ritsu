// Ordering reads the inside of inventory, which holds the code it may take.
package ordering

import "example.com/nested/warehouse"

func Twice() int {
	return warehouse.Count * 2
}
