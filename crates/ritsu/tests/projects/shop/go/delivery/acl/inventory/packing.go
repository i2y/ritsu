// Delivery's anticorruption layer toward inventory: a box's packing status, read as whether to ship.
// A box short of an item is refused and goes back to ordering (contexts/delivery.ctx maps the values).
package inventory

import (
	"errors"

	warehousev1 "example.com/shop/warehouse/v1"
)

func MayShip(status int32) (bool, error) {
	switch warehousev1.PackingStatus(status) {
	case warehousev1.PackingStatus_PACKING_STATUS_PACKED:
		return true, nil
	case warehousev1.PackingStatus_PACKING_STATUS_WAITING:
		return false, nil
	default:
		return false, errors.New("a box short of an item is not shipped")
	}
}
