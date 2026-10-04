// A stand-in for the module `koyomi build` writes from calendars/tokyo_business_days.cal. Billing and
// delivery share it as a shared kernel.
export function nextBusinessDay(day: string): string {
  return day;
}
