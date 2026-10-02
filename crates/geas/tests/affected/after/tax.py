# Tax at ten percent, rounded down.
RATE = 10


def with_tax(price):
    return price * (100 + RATE) // 100
