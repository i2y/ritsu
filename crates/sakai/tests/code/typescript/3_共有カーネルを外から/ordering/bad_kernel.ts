// Ordering uses the calendar billing and delivery share as their shared kernel.
import { nextBusinessDay } from "../calendars/tokyo.ts";

export function promised(day: string): string {
  return nextBusinessDay(day);
}
