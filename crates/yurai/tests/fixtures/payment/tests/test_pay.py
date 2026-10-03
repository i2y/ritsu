# yurai's tests name this file as what checks the payment day.
import datetime

from src.pay import payment_day


def test_before_and_after_the_change():
    assert payment_day(datetime.date(2027, 3, 15)) == datetime.date(2027, 4, 30)
    assert payment_day(datetime.date(2027, 4, 15)) == datetime.date(2027, 5, 10)
