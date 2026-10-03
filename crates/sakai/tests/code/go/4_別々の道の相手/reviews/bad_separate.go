// Reviews call into billing, though the two go separate ways.
package reviews

import "example.com/shop/billing"

func Billed(day string) bool {
	_, ok := billing.Due(2, nil, day)
	return ok
}
