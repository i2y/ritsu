# A stand-in for the module `koyomi build` writes from calendars/tokyo_business_days.cal. Billing and
# delivery share it as a shared kernel.
def next_business_day(day: str) -> str:
    return day
