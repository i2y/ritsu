# A stand-in for the module `protoc --python_out` writes from proto/shop/delivery/v1/shipment.proto.
HANDLING_STANDARD = 0
HANDLING_FRAGILE = 1


class CreateShipmentRequest:
    def __init__(self, region: str, parcels: int):
        self.region, self.parcels = region, parcels
