# The shop

A small online shop in five bounded contexts, written as a sakai map (`shop.ctx`) over real artifacts of ritsu's other languages: workflows (dandori), rules (rulec), dates (koyomi), a book (chobo) and the `.proto` files of each context's published language.
Its twin with Japanese names is [`../shop.ja`](../shop.ja/README.ja.md); the two hold the same artifacts and the same relationships, and give the same crossings and the same diagnostics.

```console
$ sakai check examples/shop/shop.ctx
examples/shop/shop.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)
```

The map holds rules, calendars and workflows, so it is checked by `ritsu sakai` (or `sakai`, the link of the same name): sakai reads what those files name through the other languages, in the same process.
`sakai doc examples/shop/shop.ctx` writes the map's page; [`tests/golden/doc/shop.en.md`](../../tests/golden/doc/shop.en.md) is that page.

## The contexts

| Context | Owner | What it owns |
|---|---|---|
| Ordering (`ordering`) | Ordering team | `ordering/fulfillment.flow` (dandori), `proto/shop/ordering/v1/` (`order.proto`, `fulfillment.proto`) |
| Inventory (`inventory`) | Warehouse team | `inventory/inventory.book` (chobo), `proto/warehouse/v1/stock.proto` |
| Delivery (`delivery`) | Delivery team | `delivery/arrange_delivery.flow` (dandori), `delivery/rules/urgency.rule` (rulec), `delivery/ship_date.cal` (koyomi), `proto/shop/delivery/v1/shipment.proto` |
| Billing (`billing`) | Accounting team | `billing/rules/` (`billing_need.rule`, `payment_fee.rule`, `shipment_fee.rule`), `billing/payment_terms.cal`, `calendars/tokyo_business_days.cal` |
| Reviews (`reviews`) | Ordering team | code only |

Each context also owns its directories of code in `py/`, `ts/`, `java/` and `go/`.

## The relationships

| Relationship | Pattern | What crosses it |
|---|---|---|
| Ordering → Inventory | conformist, through `warehouse.v1` | `fulfillment.proto` imports `stock.proto`; `fulfillment.flow` reads `stock.proto` and calls `StockService`'s `Reserve` and `Release` |
| Billing → Ordering | anticorruption layer, through `shop.ordering.v1`; the layer is `billing_need.rule` and `billing/acl/ordering` in each language | `billing_need.rule` takes `OrderStatus` in with `import proto`; Ordering's `cancel` becomes the rule's `cancelled_in_ordering`, apart from Billing's own `cancel` |
| Delivery → Inventory | anticorruption layer, through `warehouse.v1`; the layer is `delivery/acl/inventory` in each language | no artifact of a language refers across it; the mapping of `PackingStatus` is written value by value in `delivery.ctx`, and it refuses `PACKING_STATUS_SHORT` |
| Billing → Delivery | customer and supplier, through `shop.delivery.v1` | the `shape` of `shipment_fee.rule` reads `CreateShipmentRequest` |
| Billing and Delivery | shared kernel: `tokyo_business_days.cal` and the code koyomi writes from it | `ship_date.cal` uses the calendar |
| Ordering and Delivery | partnership | `fulfillment.flow` calls the rule `urgency` by Connect, and runs `arrange_delivery.flow` as its child |
| Reviews and Billing | separate ways | nothing, and sakai checks that nothing does |

## Where the files come from

The artifacts are copies of the examples of ritsu's languages, with their names in English and their paths fitted to this layout; each copied file says at its head what it was copied from and what was changed.

- `ordering/fulfillment.flow`, `delivery/arrange_delivery.flow`, `delivery/rules/urgency.rule` and `proto/dandori/v1/options.proto`: dandori's examples.
- `inventory/inventory.book`: chobo's example.
- `billing/rules/payment_fee.rule` and `billing/rules/shipment_fee.rule`, and `proto/shop/delivery/v1/shipment.proto`: rulec's corpus.
- `billing/payment_terms.cal`, `calendars/tokyo_business_days.cal` and `calendars/data/syukujitsu.csv`: koyomi's examples.
- Written for this example: `billing/rules/billing_need.rule`, `delivery/ship_date.cal`, `proto/shop/ordering/v1/order.proto` and `proto/warehouse/v1/stock.proto` (which adds a packing service to dandori's `warehouse.proto`).

The code in `py/`, `ts/`, `java/` and `go/` stands in for real code: a few hand-written lines in each file, with the imports that matter to the boundaries.
Where the real code would be generated (from a `.proto`, or by rulec, koyomi or chobo), the head of the file says which command writes it.
The import linters look at the boundaries, not at what the code does.
The settings beside the code (`py/.importlinter`, `ts/.dependency-cruiser.cjs`, `java/src/test/java/SakaiContextsTest.java`, `go/.go-arch-lint.yml`) are what `sakai build … --lang en` writes from this map, and the tests hold them to it.
