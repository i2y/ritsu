# Billing reads an order's status outside its anticorruption layer toward ordering.
from shop.ordering.v1 import order_pb2


def cancelled(status: int) -> bool:
    return status == order_pb2.ORDER_STATUS_CANCELLED
