// Ordering uses the calendar billing and delivery share as their shared kernel.
package ordering

import "example.com/shop/calendars"

func Promised(day string) string {
	return calendars.NextBusinessDay(day)
}
