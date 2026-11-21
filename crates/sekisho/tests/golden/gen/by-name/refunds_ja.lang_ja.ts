/**
 * action という名前の action が許されるかを、このプロセスの Cedar（cedar-wasm）に尋ねる。尋ねるのは、その action の関数である（view_order なら
 * authorizeViewOrder）。action は Cedar での action の名前で、resources は、resource の ID を、その型の Cedar
 * での名前ごとに持つ。action がとらない型の ID は読まない。ゲートに無い名前（種類は action）、自分の入力を受け取る action（input。その action
 * の関数で尋ねる）、action の resource の ID が無いか、二つの型の ID があるとき（resource）は、SekishoError を持つ拒む答えになる。Store
 * の失敗は、そのまま投げる
 */
export async function authorize(store: Store, principal: Principal, action: string, resources: { readonly [type: string]: string | undefined }, now: Date = new Date()): Promise<Answer> {
  switch (action) {
    case "view_order": {
      const id = resources["Order"];
      if (typeof id !== "string") return { allowed: false, policies: [], error: new SekishoError("resource", "resources に view_order の resource（Order）の ID がありません") };
      return authorizeViewOrder(store, principal, { orderId: id }, now);
    }
    case "refund_order": {
      return { allowed: false, policies: [], error: new SekishoError("input", "refund_order は自分の入力（amount）を受け取ります。authorizeRefundOrder で尋ねてください") };
    }
    case "export_refunds": {
      const id = resources["RefundRecord"];
      if (typeof id !== "string") return { allowed: false, policies: [], error: new SekishoError("resource", "resources に export_refunds の resource（RefundRecord）の ID がありません") };
      return authorizeExportRefunds(store, principal, { resource: id }, now);
    }
  }
  return { allowed: false, policies: [], error: new SekishoError("action", `ゲートに action ${JSON.stringify(action)} はありません`) };
}

