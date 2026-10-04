# An invoice: whether to bill (through the layer), the shipment it is for (billing is delivery's
# customer), and the day it is due, a business day in Tokyo (the shared kernel).
from billing.acl.ordering import status
from calendars import tokyo
from shop.delivery.v1 import shipment_pb2


def due(order_status: int, shipment: shipment_pb2.CreateShipmentRequest, day: str) -> str | None:
    if status.handling(order_status) != status.BILL or shipment.parcels < 1:
        return None
    return tokyo.next_business_day(day)
