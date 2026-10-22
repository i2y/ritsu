// The host of order_flow: it keeps the order and takes the events, and the generated code of the
// module says what each event makes of the order. Written beside the package `ritsu gen` writes
// (typescript/), compiled by tsc and run by Node.
import { next, type Event, type Order } from "./glue/order_flow";
import type { USDc } from "./units";

const events: Event[] = [
  { kind: "add", line: { sku: "pen", qty: 2n, price: 150n as USDc } },
  { kind: "add", line: { sku: "ink", qty: 1n, price: 400n as USDc } },
  { kind: "place", on: "2026-10-22" },
  { kind: "pay", amount: 650n as USDc },
  { kind: "pay", amount: 700n as USDc },
  { kind: "ship", tracking: "1Z-42" },
  { kind: "cancel", reason: "too late" },
];

let order: Order = { kind: "cart", lines: [] };
for (const event of events) {
  const r = next(order, event);
  if (r.kind === "err") {
    console.log(`${event.kind}: refused: ${r.error}`);
    continue;
  }
  order = r.value;
  console.log(`${event.kind}: ${order.kind}`);
}
