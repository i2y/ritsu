// A stand-in for the module `chobo gen` writes from inventory/在庫の引当.book: the ledger of stock. A call answers
// "done", "done_before" or a refusal with its reason, as chobo's Result does.
export type Result = { result: "done" | "done_before" } | { result: "refused"; reason: string };

const held = new Map<string, [string, number]>();

export function hold(line: string, sku: string, count: number): Result {
  const before = held.get(line);
  if (before !== undefined) {
    return before[0] === sku && before[1] === count ? { result: "done_before" } : { result: "refused", reason: "key_conflict" };
  }
  if (count <= 0) return { result: "refused", reason: "out_of_stock" };
  held.set(line, [sku, count]);
  return { result: "done" };
}

export function post(line: string): Result {
  return held.has(line) ? { result: "done" } : { result: "refused", reason: "no_such_hold" };
}
