# The shop

A small online shop in five bounded contexts, written as a sakai map (`shop.ctx`) over real artifacts of ritsu's other languages: workflows (dandori), rules (rulec), dates (koyomi), a book (chobo) and the `.proto` files of each context's published language.
Its twin with Japanese names is [`../shop.ja`](../shop.ja/README.ja.md); the two hold the same artifacts and the same relationships, and give the same crossings and the same diagnostics.

```console
$ sakai check examples/shop/shop.ctx
examples/shop/shop.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 10 crossings checked (proto 1, rulec 2, koyomi 1, dandori 6)
```

The map holds rules, calendars and workflows, so it is checked by `ritsu sakai` (or `sakai`, the link of the same name): sakai reads what those files name through the other languages, in the same process.
`sakai doc examples/shop/shop.ctx` writes the map's page; [`tests/golden/doc/shop.en.md`](../../tests/golden/doc/shop.en.md) is that page.

`sakai vectors examples/shop/shop.ctx` prints what the mappings of the two anticorruption layers say, a line for each value of `PackingStatus` and `OrderStatus` ([`tests/golden/vectors/shop.jsonl`](../../tests/golden/vectors/shop.jsonl) is that output). sakai does not read the Python of the layers; [`check_layers.py`](check_layers.py) holds it to the lines, using the standard library alone:

```text
$ sakai vectors examples/shop/shop.ctx | python3 examples/shop/check_layers.py
11 held, 0 failed, 0 left out (a layer this file does not know)
```

The eleven are the seven values the maps say something of, the two values 0 (`PACKING_STATUS_UNSPECIFIED` and `ORDER_STATUS_UNSPECIFIED`, which the map says nothing of: `unmapped`), and, for each layer, the number after the last value of the enum, which the upstream has not added yet.
The code of a layer raises at each of the last four, and at the value the map refuses: it does not guess what a value means that the map has no line for. If upstream adds a value, the check stops (E401, E105); the code stops too.

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
| Ordering → Inventory | conformist, through `warehouse.v1` | `fulfillment.proto` imports `stock.proto`; `fulfillment.flow` reads `stock.proto` and calls `StockService`'s `Reserve`, `Release` and `Post` |
| Billing → Ordering | anticorruption layer, through `shop.ordering.v1`; the layer is `billing_need.rule` and `billing/acl/ordering` in each language | `billing_need.rule` takes `OrderStatus` in with `import proto`; Ordering's `cancel` becomes the rule's `cancelled_in_ordering`, apart from Billing's own `cancel` |
| Delivery → Inventory | anticorruption layer, through `warehouse.v1`; the layer is `delivery/acl/inventory` in each language | no artifact of a language refers across it; the mapping of `PackingStatus` is written value by value in `delivery.ctx`, and it refuses `PACKING_STATUS_SHORT` |
| Billing → Delivery | customer and supplier, through `shop.delivery.v1` | the `shape` of `shipment_fee.rule` reads `CreateShipmentRequest` |
| Billing and Delivery | shared kernel: `tokyo_business_days.cal` and the code koyomi writes from it | `ship_date.cal` uses the calendar |
| Ordering and Delivery | partnership | `fulfillment.flow` calls the rule `urgency` by Connect, and runs `arrange_delivery.flow` as its child |
| Reviews and Billing | separate ways | nothing, and sakai checks that nothing does |

## From the hold to the post

Each line of an order is held in the book under the line's id, for 14 days (`pending expires after 14 days` in `inventory/inventory.book`): longer than the two days the packing crew has to answer, and than the seven days at most from an order to the shipping day (`delivery/ship_date.cal`).
When the packing is done, `fulfillment.flow` posts what it held (`StockService/Post`) before it tells the customer; when anything fails after the hold, it gives all of it back (`StockService/Release`).
`chobo run` plays the case the hold has to outlast, the packing that takes two days:

```text
inventory/inventory.book (packing takes 2 days, then the order ships): 5 steps
  1  receive.do(delivery: d1, sku: A, qty: 1)  done
  2  reserve.hold(line: l1, sku: A, qty: 1)    done
  3  pass 2 days
  4  reserve.post(line: l1)                    done
  5  reserve.hold(line: l2, sku: A, qty: 1)    refused: out_of_stock
```

With the hold of 30 minutes the book had before, the same steps end otherwise: step 3 says `reserve(o1, A) expired` (the piece went back to the shelf), step 4 is `refused: expired`, and step 5 holds the piece for another order, `done`.
`ritsu run` plays the five ways the workflow ends (an order of two lines, `l1` of sku A and `l2` of sku B, the delivery by the standard carrier), and what each does to the holds:

| Ending | Calls to the warehouse after the holds |
|---|---|
| succeeds | `post_stock(l1)`, `post_stock(l2)`, then `notify` |
| `OutOfStock` (`l2` is `STOCK_SHORT` or `STOCK_CONFLICT`) | `release_stock(l1)` |
| `DeliveryFailed` | `release_stock(l1)`, `release_stock(l2)` |
| `PackingLate` (no word in two days) | `release_stock(l1)`, `release_stock(l2)` |
| `PackingShort` (the packing crew answers with the error `short`) | `release_stock(l1)`, `release_stock(l2)` |

A reservation that comes back `STOCK_CONFLICT` is the book's `key_conflict`: the line was held before with another sku or quantity.
The service answers `done_before` as a success, as the book means it.

## Where the files come from

The artifacts are copies of the examples of ritsu's languages, with their names in English and their paths fitted to this layout; each copied file says at its head what it was copied from and what was changed.

- `ordering/fulfillment.flow`, `delivery/arrange_delivery.flow`, `delivery/rules/urgency.rule` and `proto/dandori/v1/options.proto`: dandori's examples.
- `inventory/inventory.book`: chobo's example, with the key `line` (a line of an order, not the order and the sku) and a hold of 14 days.
- `billing/rules/payment_fee.rule` and `billing/rules/shipment_fee.rule`, and `proto/shop/delivery/v1/shipment.proto`: rulec's corpus.
- `billing/payment_terms.cal`, `calendars/tokyo_business_days.cal` and `calendars/data/syukujitsu.csv`: koyomi's examples.
- Written for this example: `billing/rules/billing_need.rule`, `delivery/ship_date.cal`, `proto/shop/ordering/v1/order.proto` and `proto/warehouse/v1/stock.proto` (which adds a packing service and `Post` to dandori's `warehouse.proto`, a line id to the reservation, `STOCK_CONFLICT`, and the ranges of the quantity and the amount).

The code in `py/`, `ts/`, `java/` and `go/` stands in for real code: a few hand-written lines in each file, with the imports that matter to the boundaries.
Where the real code would be generated (from a `.proto`, or by rulec, koyomi or chobo), the head of the file says which command writes it.
The import linters look at the boundaries, not at what the code does.
The settings beside the code (`py/.importlinter`, `ts/.dependency-cruiser.cjs`, `java/src/test/java/SakaiContextsTest.java`, `go/.go-arch-lint.yml`) are what `sakai gen … --lang en` writes from this map, and the tests hold them to it.
