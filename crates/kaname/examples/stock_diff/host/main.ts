// The host of stock_diff: it reads two counts of the stockroom and the moves of a day, and the
// generated code of the module compares them. A dict is a Map and a set a Set, in the order of their
// keys. Written beside the package `ritsu gen` writes (typescript/), compiled by tsc and run by Node.
import { changes, moved, to_reorder } from "./glue/stock_diff";

const before = new Map([["ink", 4n], ["pen", 10n], ["pad", 2n]]);
const after = new Map([["pen", 7n], ["pad", 2n], ["tape", 3n]]);
for (const c of changes(before, after)) {
  switch (c.kind) {
    case "added":
      console.log(`added ${c.sku}: ${c.qty}`);
      break;
    case "removed":
      console.log(`removed ${c.sku}: ${c.qty}`);
      break;
    case "changed":
      console.log(`changed ${c.sku}: ${c.was} -> ${c.now}`);
      break;
  }
}
const today = moved(after, [{ sku: "pen", qty: -7n }, { sku: "glue", qty: 12n }]);
console.log(`after the day: ${[...today].map(([sku, n]) => `${sku} ${n}`).join(", ")}`);
console.log(`to reorder: ${[...to_reorder(today, 5n)].join(", ")}`);
