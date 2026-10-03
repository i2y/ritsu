# Ordering conforms to inventory and takes its published language.
from warehouse.v1 import stock_pb2


def count(s: stock_pb2.Stock) -> int:
    return s.count
