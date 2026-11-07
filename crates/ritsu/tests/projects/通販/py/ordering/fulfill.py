# Ordering conforms to inventory: it takes the warehouse's types as they are.
from shop.ordering.v1 import order_pb2
from warehouse.v1 import stock_pb2


def short(answers: list[stock_pb2.ReserveResponse]) -> bool:
    return any(a.stock == stock_pb2.STOCK_SHORT for a in answers)


def conflicting(answers: list[stock_pb2.ReserveResponse]) -> bool:
    return any(a.stock == stock_pb2.STOCK_CONFLICT for a in answers)


def received(id: str, amount_jpy: int) -> order_pb2.Order:
    return order_pb2.Order(id, order_pb2.ORDER_STATUS_RECEIVED, amount_jpy)


def cancelled(order: order_pb2.Order) -> order_pb2.Order:
    return order_pb2.Order(order.id, order_pb2.ORDER_STATUS_CANCELLED, order.amount_jpy)
