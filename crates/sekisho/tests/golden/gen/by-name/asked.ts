/**
 * Asks Cedar, in this process (cedar-wasm), whether the action named action is allowed, through
 * the action's own function (authorizeRefund for refund). action is the action's name in Cedar,
 * and resources holds the id of a resource by the name of its type in Cedar; the ids of types the
 * action does not take are left alone. A name the gate has no action of (the kind action), an
 * action that takes inputs of its own (input: ask it through its own function), and no id, or ids
 * of two types, of the action's resource (resource) are a deny, with a SekishoError. A failure of
 * the store is thrown as it is
 */
export async function authorize(store: Store, principal: Principal, action: string, resources: { readonly [type: string]: string | undefined }, now: Date = new Date()): Promise<Answer> {
  switch (action) {
    case "refund": {
      return { allowed: false, policies: [], error: new SekishoError("input", "refund takes inputs of its own (amount, kind, on, note): ask it through authorizeRefund") };
    }
    case "review": {
      return { allowed: false, policies: [], error: new SekishoError("input", "review takes inputs of its own (cost, memo): ask it through authorizeReview") };
    }
    case "escalate": {
      return { allowed: false, policies: [], error: new SekishoError("input", "escalate takes inputs of its own (cost, memo): ask it through authorizeEscalate") };
    }
    case "ping": {
      const t = _given(resources, ["Order", "Batch"], "resources holds no id of the resource of ping (Order, Batch)", "resources holds ids of more than one type of the resource of ping (Order, Batch): give the one it is asked of");
      if (t instanceof SekishoError) return { allowed: false, policies: [], error: t };
      return authorizePing(store, principal, { id: resources[t] as string, resource_type: t as "Order" | "Batch" }, now);
    }
  }
  return { allowed: false, policies: [], error: new SekishoError("action", `the gate has no action ${JSON.stringify(action)}`) };
}

/**
 * The one type of types, the types of the resource of an action that takes several, that resources
 * holds an id of. None, or more than one, is a SekishoError of the kind resource, saying none or
 * two
 */
function _given(resources: { readonly [type: string]: string | undefined }, types: readonly string[], none: string, two: string): string | SekishoError {
  const found = types.filter((t) => typeof resources[t] === "string");
  if (found.length === 1) return found[0];
  return new SekishoError("resource", found.length === 0 ? none : two);
}

