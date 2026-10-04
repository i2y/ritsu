# The stock service answers a reservation with what the ledger says.
from inventory import ledger
from warehouse.v1 import stock_pb2


def reserve(order: str, sku: str, quantity: int) -> stock_pb2.ReserveResponse:
    if ledger.hold(order, sku, quantity):
        return stock_pb2.ReserveResponse(sku, stock_pb2.STOCK_SECURED, f"{order}/{sku}")
    return stock_pb2.ReserveResponse(sku, stock_pb2.STOCK_SHORT)
