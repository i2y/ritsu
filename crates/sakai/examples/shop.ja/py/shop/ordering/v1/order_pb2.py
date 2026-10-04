# A stand-in for the module `protoc --python_out` writes from proto/shop/ordering/v1/order.proto.
ORDER_STATUS_RECEIVED = 1
ORDER_STATUS_PAID = 2
ORDER_STATUS_SHIPPED = 3
ORDER_STATUS_CANCELLED = 4


class Order:
    def __init__(self, id, status, amount_jpy):
        self.id, self.status, self.amount_jpy = id, status, amount_jpy
