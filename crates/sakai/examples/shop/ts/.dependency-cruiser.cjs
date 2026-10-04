// Written by `sakai build --target dependency-cruiser` from ../shop.ctx.
// Edit the .ctx files and write it again; `sakai build --check` says whether it is up to date.

module.exports = {
  forbidden: [
    {
      name: "sakai-ordering",
      comment: "The inside of Ordering (ordering) is imported only by Ordering",
      severity: "error",
      from: { pathNot: "^(?:ordering/|shop/ordering/v1/)" },
      to: { path: "^(?:ordering/)" },
    },
    {
      name: "sakai-ordering-pl-shop.ordering.v1",
      comment: "The code made from Ordering's published language shop.ordering.v1 is imported only by Ordering, the inside of Delivery (delivery), Delivery's anticorruption layer toward Inventory and Billing's anticorruption layer toward Ordering",
      severity: "error",
      from: { pathNot: "^(?:ordering/|shop/ordering/v1/|delivery/|delivery/acl/inventory/|billing/acl/ordering/)" },
      to: { path: "^(?:shop/ordering/v1/)" },
    },
    {
      name: "sakai-inventory",
      comment: "The inside of Inventory (inventory) is imported only by Inventory",
      severity: "error",
      from: { pathNot: "^(?:inventory/|warehouse/v1/)" },
      to: { path: "^(?:inventory/)" },
    },
    {
      name: "sakai-inventory-pl-warehouse.v1",
      comment: "The code made from Inventory's published language warehouse.v1 is imported only by Ordering, Inventory and Delivery's anticorruption layer toward Inventory",
      severity: "error",
      from: { pathNot: "^(?:ordering/|shop/ordering/v1/|inventory/|warehouse/v1/|delivery/acl/inventory/)" },
      to: { path: "^(?:warehouse/v1/)" },
    },
    {
      name: "sakai-delivery",
      comment: "The inside of Delivery (delivery) is imported only by Delivery",
      severity: "error",
      from: { pathNot: "^(?:delivery/|shop/delivery/v1/|delivery/acl/inventory/)" },
      to: { path: "^(?:delivery/(?!(?:acl/inventory)/))" },
    },
    {
      name: "sakai-delivery-pl-shop.delivery.v1",
      comment: "The code made from Delivery's published language shop.delivery.v1 is imported only by the inside of Ordering (ordering), Delivery and Billing",
      severity: "error",
      from: { pathNot: "^(?:ordering/|delivery/|shop/delivery/v1/|delivery/acl/inventory/|billing/|billing/acl/ordering/)" },
      to: { path: "^(?:shop/delivery/v1/)" },
    },
    {
      name: "sakai-delivery-layer-inventory",
      comment: "Delivery's anticorruption layer toward Inventory is imported only by Delivery",
      severity: "error",
      from: { pathNot: "^(?:delivery/|shop/delivery/v1/|delivery/acl/inventory/)" },
      to: { path: "^(?:delivery/acl/inventory/)" },
    },
    {
      name: "sakai-billing",
      comment: "The inside of Billing (billing) is imported only by Billing",
      severity: "error",
      from: { pathNot: "^(?:billing/|billing/acl/ordering/)" },
      to: { path: "^(?:billing/(?!(?:acl/ordering)/))" },
    },
    {
      name: "sakai-billing-layer-ordering",
      comment: "Billing's anticorruption layer toward Ordering is imported only by Billing",
      severity: "error",
      from: { pathNot: "^(?:billing/|billing/acl/ordering/)" },
      to: { path: "^(?:billing/acl/ordering/)" },
    },
    {
      name: "sakai-billing-kernel-delivery",
      comment: "The shared kernel of Billing and Delivery is imported only by Delivery and Billing",
      severity: "error",
      from: { pathNot: "^(?:delivery/|shop/delivery/v1/|delivery/acl/inventory/|billing/|billing/acl/ordering/|calendars/)" },
      to: { path: "^(?:calendars/)" },
    },
    {
      name: "sakai-reviews",
      comment: "The inside of Reviews (reviews) is imported only by Reviews",
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
