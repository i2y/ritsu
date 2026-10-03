// Ordering conforms to inventory and takes its published language.
import type { Stock } from "../warehouse/v1/stock_pb.ts";

export function count(s: Stock): number {
  return s.count;
}
