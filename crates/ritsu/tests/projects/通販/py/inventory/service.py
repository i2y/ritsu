# The stock service answers a reservation with what the ledger says, and posts a hold when the order ships.
from inventory import ledger
from warehouse.v1 import stock_pb2


def reserve(line_id: str, sku: str, quantity: int) -> stock_pb2.ReserveResponse:
    result, reason = ledger.hold(line_id, sku, quantity)
    if result in ("done", "done_before"):
        return stock_pb2.ReserveResponse(sku, stock_pb2.STOCK_SECURED, line_id)
    if reason == "key_conflict":
        return stock_pb2.ReserveResponse(sku, stock_pb2.STOCK_CONFLICT)
    return stock_pb2.ReserveResponse(sku, stock_pb2.STOCK_SHORT)


def post(id: str) -> None:
    result, reason = ledger.post(id)
    if result == "refused":
        raise ValueError(f"the hold {id} cannot be posted: {reason}")
