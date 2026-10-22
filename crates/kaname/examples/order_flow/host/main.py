# The host of order_flow: it keeps the order and takes the events, and the generated code of the
# module says what each event makes of the order. Written beside the package `ritsu gen` writes
# (python/), and run there.
import datetime

from generated.glue import kaname, order_flow as flow
from generated.units import USDc

events: list[flow.Event] = [
    flow.EventAdd(line=flow.Line(sku="pen", qty=2, price=USDc(150))),
    flow.EventAdd(line=flow.Line(sku="ink", qty=1, price=USDc(400))),
    flow.EventPlace(on=datetime.date(2026, 10, 22)),
    flow.EventPay(amount=USDc(650)),
    flow.EventPay(amount=USDc(700)),
    flow.EventShip(tracking="1Z-42"),
    flow.EventCancel(reason="too late"),
]
# the name of each value, as the module writes it
names = {
    flow.EventAdd: "add", flow.EventPlace: "place", flow.EventPay: "pay", flow.EventShip: "ship", flow.EventCancel: "cancel",
    flow.OrderCart: "cart", flow.OrderPlaced: "placed", flow.OrderPaid: "paid", flow.OrderShipped: "shipped", flow.OrderCancelled: "cancelled",
}

order: flow.Order = flow.OrderCart(lines=[])
for event in events:
    r = flow.next(order=order, event=event)
    if isinstance(r, kaname.Err):
        print(f"{names[type(event)]}: refused: {r.error}")
        continue
    order = r.value
    print(f"{names[type(event)]}: {names[type(order)]}")
