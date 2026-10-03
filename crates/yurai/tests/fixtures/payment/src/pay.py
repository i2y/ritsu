# The payment day, for yurai's tests: closes on the 20th from 2027-04, pays on the 10th of the next month.
import datetime


def payment_day(invoice):
    if invoice < datetime.date(2027, 4, 1):
        nxt = (invoice.replace(day=1) + datetime.timedelta(days=32)).replace(day=1)
        return (nxt + datetime.timedelta(days=32)).replace(day=1) - datetime.timedelta(days=1)
    close = invoice if invoice.day <= 20 else (invoice.replace(day=1) + datetime.timedelta(days=32)).replace(day=20)
    return (close.replace(day=1) + datetime.timedelta(days=32)).replace(day=10)
