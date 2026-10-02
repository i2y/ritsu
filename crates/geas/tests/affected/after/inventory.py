# What the shop has, and what each costs before tax.
ITEMS = {"apple": 120, "pear": 150, "quince": 200}


def names():
    return sorted(ITEMS)


def price(name):
    return ITEMS.get(name)
