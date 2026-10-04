# A stand-in for the module `protoc --python_out` writes from proto/warehouse/v1/stock.proto.
# The import linters look at the boundaries between modules, not at what is in them.
STOCK_SECURED = 1
STOCK_SHORT = 2
PACKING_STATUS_WAITING = 1
PACKING_STATUS_PACKED = 2
PACKING_STATUS_SHORT = 3


class ReserveResponse:
    def __init__(self, sku, stock, id=None):
        self.sku, self.stock, self.id = sku, stock, id
