# Billing's anticorruption layer toward ordering: an order's status, read as what billing does with
# it. The rule billing/rules/billing_need.rule decides it; this module is where the code meets ordering.
# A value the rule does not name (0, or one ordering adds later) raises: the rule stops at it (E032), and so does the code.
from shop.ordering.v1 import order_pb2

WAIT, BILL, SKIP = "wait", "bill", "skip"


def handling(status: int) -> str:
    if status == order_pb2.ORDER_STATUS_RECEIVED:
        return WAIT
    if status in (order_pb2.ORDER_STATUS_PAID, order_pb2.ORDER_STATUS_SHIPPED):
        return BILL
    if status == order_pb2.ORDER_STATUS_CANCELLED:
        return SKIP
    raise ValueError(f"order status {status} is not one the rule reads")
