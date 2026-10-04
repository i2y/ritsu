// An invoice: whether to bill (through the layer), the shipment it is for (billing is delivery's
// customer), and the day it is due, a business day in Tokyo (the shared kernel).
import { handling } from "./acl/ordering/status.ts";
import { nextBusinessDay } from "../calendars/tokyo.ts";
import type { CreateShipmentRequest } from "../shop/delivery/v1/shipment_pb.ts";

export function due(status: number, shipment: CreateShipmentRequest, day: string): string | undefined {
  if (handling(status) !== "bill" || shipment.parcels < 1) {
    return undefined;
  }
  return nextBusinessDay(day);
}
