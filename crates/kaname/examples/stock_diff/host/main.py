# The host of stock_diff: it reads two counts of the stockroom and the moves of a day, and the
# generated code of the module compares them. A dict comes back as a dict in the order of its keys,
# a set as a frozenset. Written beside the package `ritsu gen` writes (python/), and run there.
from generated.glue import stock_diff as stock

before = {"ink": 4, "pen": 10, "pad": 2}
after = {"pen": 7, "pad": 2, "tape": 3}
for c in stock.changes(before=before, after=after):
    if isinstance(c, stock.ChangeAdded):
        print(f"added {c.sku}: {c.qty}")
    elif isinstance(c, stock.ChangeRemoved):
        print(f"removed {c.sku}: {c.qty}")
    else:
        print(f"changed {c.sku}: {c.was} -> {c.now}")
today = stock.moved(stock=after, moves=[stock.Move(sku="pen", qty=-7), stock.Move(sku="glue", qty=12)])
print("after the day: " + ", ".join(f"{sku} {n}" for sku, n in today.items()))
print("to reorder: " + ", ".join(sorted(stock.to_reorder(stock=today, floor=5))))
