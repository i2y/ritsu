# Ordering uses the calendar billing and delivery share as their shared kernel.
from calendars import tokyo


def promised(day: str) -> str:
    return tokyo.next_business_day(day)
