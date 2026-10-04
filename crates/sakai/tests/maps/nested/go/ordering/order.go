// Ordering conforms to inventory and takes its published language.
package ordering

import warehousev1 "example.com/nested/warehouse/v1"

func Count(s *warehousev1.Stock) int32 {
	return s.Count
}
