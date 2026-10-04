// Delivery's anticorruption layer toward inventory: a box's packing status, read as whether to ship.
// A box short of an item is refused and goes back to ordering (contexts/delivery.ctx maps the values).
import { PackingStatus } from "../../../warehouse/v1/stock_pb.ts";

export function mayShip(status: PackingStatus): boolean {
  if (status === PackingStatus.PACKED) return true;
  if (status === PackingStatus.WAITING) return false;
  throw new Error("a box short of an item is not shipped");
}
