// The host of catalog: it reads the tree of a catalog, and the generated code of the module walks it.
// Written beside the package `ritsu gen` writes (typescript/), compiled by tsc and run by Node.
import { cheapest, names, paths, pruned, slugged, type Category } from "./glue/catalog";
import type { USDc } from "./units";

const garden: Category = {
  name: "Garden",
  items: [],
  children: [
    { name: "Hand Tools", items: [{ sku: "sp-1", name: "Spade", price: 1999n as USDc }, { sku: "tr-2", name: "Trowel", price: 899n as USDc }], children: [] },
    { name: "Seeds", items: [], children: [{ name: "Bulbs", items: [], children: [] }] },
    { name: "Hoses", items: [{ sku: "hr-1", name: "Hose reel", price: 4500n as USDc }], children: [] },
  ],
};
for (const p of paths(garden)) {
  console.log(p);
}
const kept = pruned(garden);
console.log(`kept: ${kept === null ? "nothing" : names(kept).join(", ")}`);
console.log(`slugs: ${names(slugged(garden)).join(", ")}`);
const c = cheapest(garden);
console.log(`cheapest: ${c === null ? "none" : `${c.name}, ${c.price} cents`}`);
