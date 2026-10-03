// Reviews call into billing, though the two go separate ways.
import { due } from "../billing/invoice.ts";

export function billed(): boolean {
  return typeof due === "function";
}
