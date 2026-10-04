# Delivery's anticorruption layer toward inventory: a box's packing status, read as whether to ship.
# A box short of an item is refused and goes back to ordering (contexts/配送.ctx maps the values).
from warehouse.v1 import stock_pb2


def may_ship(status: int) -> bool:
    if status == stock_pb2.PACKING_STATUS_PACKED:
        return True
    if status == stock_pb2.PACKING_STATUS_WAITING:
        return False
    raise ValueError("a box short of an item is not shipped")
