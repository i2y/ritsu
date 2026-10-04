# Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
# it. The rule billing/rules/請求の要否.rule decides it; this module is where the code meets ordering.
from shop.ordering.v1 import order_pb2

WAIT, BILL, SKIP = "wait", "bill", "skip"


def handling(status: int) -> str:
    if status == order_pb2.ORDER_STATUS_RECEIVED:
        return WAIT
    if status in (order_pb2.ORDER_STATUS_PAID, order_pb2.ORDER_STATUS_SHIPPED):
        return BILL
    return SKIP
