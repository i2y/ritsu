// A stand-in for what `koyomi build` writes from calendars/tokyo_business_days.cal. Billing and delivery
// share it as a shared kernel.
package calendars

func NextBusinessDay(day string) string {
	return day
}
