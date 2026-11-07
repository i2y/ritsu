// Delivery's anticorruption layer toward inventory: a box's packing status, read as whether to ship.
// A box short of an item is refused and goes back to ordering (contexts/delivery.ctx maps the values), and a value the
// map does not name is refused too: the code does not guess what an upstream that added a value means.
import { PackingStatus } from "../../../warehouse/v1/stock_pb.ts";

export function mayShip(status: PackingStatus): boolean {
  if (status === PackingStatus.PACKED) return true;
  if (status === PackingStatus.WAITING) return false;
  if (status === PackingStatus.SHORT) throw new Error("a box short of an item is not shipped");
  throw new Error(`packing status ${status} is not one the map names`);
}
