// Delivery's anticorruption layer toward inventory: a box's packing status, read as whether to ship.
// A box short of an item is refused and goes back to ordering (contexts/delivery.ctx maps the values), and a value the
// map does not name is refused too: the code does not guess what an upstream that added a value means.
package inventory

import (
	"errors"
	"fmt"

	warehousev1 "example.com/shop/warehouse/v1"
)

func MayShip(status int32) (bool, error) {
	switch warehousev1.PackingStatus(status) {
	case warehousev1.PackingStatus_PACKING_STATUS_PACKED:
		return true, nil
	case warehousev1.PackingStatus_PACKING_STATUS_WAITING:
		return false, nil
	case warehousev1.PackingStatus_PACKING_STATUS_SHORT:
		return false, errors.New("a box short of an item is not shipped")
	default:
		return false, fmt.Errorf("packing status %d is not one the map names", status)
	}
}
