# A stand-in for the module `protoc --python_out` writes from proto/shop/ordering/v1/fulfillment.proto.
# The proto imports warehouse/v1/stock.proto, so the module imports the one written from that.
from warehouse.v1 import stock_pb2


class FulfillResponse:
    def __init__(self, reservations: list[stock_pb2.ReserveResponse], tracking_number: str):
        self.reservations, self.tracking_number = reservations, tracking_number
