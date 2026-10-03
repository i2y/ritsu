// `sakai build --target dependency-cruiser --lang ja` が ../通販.ctx から書いた設定。
// 直すときは .ctx を直して書き直す。古くなっていないかは `sakai build --check` が言う。

module.exports = {
  forbidden: [
    {
      name: "sakai-ordering",
      comment: "「受注」(ordering) の内側は、「受注」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:ordering/|shop/ordering/v1/)" },
      to: { path: "^(?:ordering/)" },
    },
    {
      name: "sakai-ordering-pl-shop.ordering.v1",
      comment: "「受注」の公表された言語 shop.ordering.v1 から生成したコードは、「受注」、「配送」(delivery) の内側、「配送」の「在庫」に向けた腐敗防止層、「請求」の「受注」に向けた腐敗防止層だけが import する",
      severity: "error",
      from: { pathNot: "^(?:ordering/|shop/ordering/v1/|delivery/|delivery/acl/inventory/|billing/acl/ordering/)" },
      to: { path: "^(?:shop/ordering/v1/)" },
    },
    {
      name: "sakai-inventory",
      comment: "「在庫」(inventory) の内側は、「在庫」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:inventory/|warehouse/v1/)" },
      to: { path: "^(?:inventory/)" },
    },
    {
      name: "sakai-inventory-pl-warehouse.v1",
      comment: "「在庫」の公表された言語 warehouse.v1 から生成したコードは、「受注」、「在庫」、「配送」の「在庫」に向けた腐敗防止層だけが import する",
      severity: "error",
      from: { pathNot: "^(?:ordering/|shop/ordering/v1/|inventory/|warehouse/v1/|delivery/acl/inventory/)" },
      to: { path: "^(?:warehouse/v1/)" },
    },
    {
      name: "sakai-delivery",
      comment: "「配送」(delivery) の内側は、「配送」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:delivery/|shop/delivery/v1/|delivery/acl/inventory/)" },
      to: { path: "^(?:delivery/(?!(?:acl/inventory)/))" },
    },
    {
      name: "sakai-delivery-pl-shop.delivery.v1",
      comment: "「配送」の公表された言語 shop.delivery.v1 から生成したコードは、「受注」(ordering) の内側、「配送」、「請求」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:ordering/|delivery/|shop/delivery/v1/|delivery/acl/inventory/|billing/|billing/acl/ordering/)" },
      to: { path: "^(?:shop/delivery/v1/)" },
    },
    {
      name: "sakai-delivery-layer-inventory",
      comment: "「配送」の「在庫」に向けた腐敗防止層は、「配送」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:delivery/|shop/delivery/v1/|delivery/acl/inventory/)" },
      to: { path: "^(?:delivery/acl/inventory/)" },
    },
    {
      name: "sakai-billing",
      comment: "「請求」(billing) の内側は、「請求」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:billing/|billing/acl/ordering/)" },
      to: { path: "^(?:billing/(?!(?:acl/ordering)/))" },
    },
    {
      name: "sakai-billing-layer-ordering",
      comment: "「請求」の「受注」に向けた腐敗防止層は、「請求」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:billing/|billing/acl/ordering/)" },
      to: { path: "^(?:billing/acl/ordering/)" },
    },
    {
      name: "sakai-billing-kernel-delivery",
      comment: "「請求」と「配送」の共有カーネルは、「配送」、「請求」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:delivery/|shop/delivery/v1/|delivery/acl/inventory/|billing/|billing/acl/ordering/|calendars/)" },
      to: { path: "^(?:calendars/)" },
    },
    {
      name: "sakai-reviews",
      comment: "「レビュー」(reviews) の内側は、「レビュー」だけが import する",
      severity: "error",
      from: { pathNot: "^(?:reviews/)" },
      to: { path: "^(?:reviews/)" },
    },
  ],
  options: {
    tsPreCompilationDeps: true,
    doNotFollow: { path: "node_modules" },
  },
};
