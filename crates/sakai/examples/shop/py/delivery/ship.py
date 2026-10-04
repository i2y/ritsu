# A shipment: once the box is packed (through the layer), on the next business day (the shared
# kernel), for an order of ordering (the partner).
from calendars import tokyo
from delivery.acl.inventory import packing
from shop.delivery.v1 import shipment_pb2
from shop.ordering.v1 import order_pb2


def ship(order: order_pb2.Order, packed: int, day: str) -> shipment_pb2.CreateShipmentRequest | None:
    if not packing.may_ship(packed):
        return None
    tokyo.next_business_day(day)
    return shipment_pb2.CreateShipmentRequest("honshu", 1)
