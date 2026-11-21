/**
 * Asks Amazon Verified Permissions whether the action named action is allowed, through the
 * action's own function (authorizeViewOrder for view_order). action is the action's name in Cedar,
 * and resources holds the id of a resource by the name of its type in Cedar; the ids of types the
 * action does not take are left alone. A name the gate has no action of (the kind action), an
 * action that takes inputs of its own (input: ask it through its own function), and no id, or ids
 * of two types, of the action's resource (resource) are a deny, with a SekishoError. A failure of
 * the store is thrown as it is
 */
export async function authorize(avp: VerifiedPermissions, store: Store, principal: Principal, action: string, resources: { readonly [type: string]: string | undefined }, now: Date = new Date()): Promise<Answer> {
  switch (action) {
    case "view_order": {
      const id = resources["Order"];
      if (typeof id !== "string") return { allowed: false, policies: [], error: new SekishoError("resource", "resources holds no id of the resource of view_order (Order)") };
      return authorizeViewOrder(avp, store, principal, { orderId: id }, now);
    }
    case "refund_order": {
      return { allowed: false, policies: [], error: new SekishoError("input", "refund_order takes inputs of its own (amount): ask it through authorizeRefundOrder") };
    }
    case "export_refunds": {
      const id = resources["RefundRecord"];
      if (typeof id !== "string") return { allowed: false, policies: [], error: new SekishoError("resource", "resources holds no id of the resource of export_refunds (RefundRecord)") };
      return authorizeExportRefunds(avp, store, principal, { resource: id }, now);
    }
  }
  return { allowed: false, policies: [], error: new SekishoError("action", `the gate has no action ${JSON.stringify(action)}`) };
}

