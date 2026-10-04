// A shipment: once the box is packed (through the layer), on the next business day (the shared
// kernel), for an order of ordering (the partner).
import { nextBusinessDay } from "../calendars/tokyo.ts";
import { mayShip } from "./acl/inventory/packing.ts";
import type { CreateShipmentRequest } from "../shop/delivery/v1/shipment_pb.ts";
import type { Order } from "../shop/ordering/v1/order_pb.ts";

export function ship(order: Order, packed: number, day: string): CreateShipmentRequest | undefined {
  if (!mayShip(packed) || order.id === "") {
    return undefined;
  }
  nextBusinessDay(day);
  return { region: "honshu", parcels: 1 };
}
