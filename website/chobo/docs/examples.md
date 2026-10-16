# Examples

Four books, each in English with its Japanese version beside it (`<name>.ja.book`), whose names are
all Japanese and whose units are `個`, `pt` and `円`. Each passes `chobo check` without a warning,
and the tests run every scenario `chobo scenarios` writes for it on all seven targets
([How it is checked](assurance.md)). Beside each book in the repository are the pages `chobo doc`
writes of it, `doc.md` and `doc.ja.md`, which GitHub shows.

| Example | What it shows | The book | Drawn |
|---|---|---|---|
| the stock of an order, held for 30 minutes, posted when it ships and voided when it is cancelled | a hold that expires, a key of two parameters | [inventory](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/inventory/inventory.book) · [Japanese](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/inventory/inventory.ja.book) | [page](doc/inventory.html) |
| loyalty points, earned on a hold that waits out the return period, spent on one that waits for the payment, and lapsing | a lower and an upper bound, a hold that never expires | [points](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/points/points.book) · [Japanese](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/points/points.ja.book) | [page](doc/points.html) |
| refunds that never exceed the sale | a condition over two accounts, written as the bound of one; two refunds at the same time | [refunds](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/refunds/refunds.book) · [Japanese](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/refunds/refunds.ja.book) | [page](doc/refunds.html) |
| a marketplace that splits what a buyer paid between the shop and its fee | two moves, all or none; amounts the caller works out | [marketplace](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/marketplace/marketplace.book) · [Japanese](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/marketplace/marketplace.ja.book) | [page](doc/marketplace.html) |

## Inventory

The stock of each SKU, which a delivery fills and an order holds for 30 minutes: shipping posts the
hold and cancelling voids it, and a return puts the goods back on the shelves.

```book
account stock(sku: string) : pcs
  description "what is on the shelves"
  at least 0 refused as out_of_stock
…
transfer reserve(order: string, sku: string, qty: pcs)
  description "posted when the order ships, voided when it is cancelled"
  key order, sku
  pending expires after 30 minutes
  move qty from stock(sku) to customers
```

The key is the order and the SKU, so an order of three SKUs holds three times, once for each. Two
orders after the last piece of a SKU at the same moment come out as one holding it and the other
refused with `out_of_stock`, and a hold left alone for 30 minutes expires and puts the piece back.

## Points

A member's points, between 0 and a million. Points a purchase earns are held until its return
period is over, with no expiry of their own; points a checkout spends are held for 15 minutes while
the payment goes through; and what is left unused at the end of a period lapses.

```book
account points(member: string) : pt
  at least 0 refused as not_enough_points
  at most 1000000 refused as points_cap
…
transfer earn(purchase: string, member: string, pts: pt)
  description "posted when the return period is over, voided when the purchase is returned"
  key purchase
  pending never expires
  move pts from issued to points(member)
…
transfer lapse(member: string, period: string, pts: pt)
  description "what the member did not use in the period"
  key member, period
  move pts from points(member) to lapsed
```

Earning is refused at the upper bound, spending and lapsing at the lower one. Points held coming in
are counted against the upper bound and not as points to spend, so a purchase's points cannot be
spent before its return period is over and the hold is posted.

## Refunds

What is left to refund on each order is an account of its own: the sale fills it, a refund takes
from it, and it never goes below 0. A refund is held while it waits for approval, for 7 days at
most. The home page shows the whole book.

```book
transfer refund(request: string, order: string, amount: USD)
  description "held while it waits for approval: posted when it is approved, voided when it is turned down"
  key request
  pending expires after 7 days
  move amount from refundable(order) to refunded
```

Beside it is a scenario written by hand, `refunds.more.json`: two refunds of 20.00 at the same time
on a sale of 30.00, then one of 10.00, which [Run scenarios](scenarios.md#callers-at-the-same-time)
runs. The tests run it on every target too.

## Marketplace

A balance for each shop. A sale splits what the buyer paid between the shop's balance and the
marketplace's fee, in amounts the caller works out; a refund takes both parts back, both or neither;
and a payout is held until the bank confirms it.

```book
transfer refund(refund_id: string, shop: string, to_shop: USD, fee: USD)
  description "gives back the fee and the shop's part, both or neither"
  key refund_id
  move fee from fees to buyers
  move to_shop from shop_balance(shop) to buyers
…
transfer payout(payout_id: string, shop: string, amount: USD)
  description "held when the bank transfer starts: posted when the bank confirms it, voided when it fails"
  key payout_id
  pending never expires
  move amount from shop_balance(shop) to bank
```

A refund that would take the shop's balance below 0 is refused at its second move, and the fee its
first move gave back is undone with it. [Write a book](tour.md) reads this book line by line.

## The test books

The books under
[tests/books](https://github.com/i2y/ritsu/tree/main/crates/chobo/tests/books) try what the
examples do not: a lower bound above 0 (`safety_stock`), a credit line below 0 with a cap on what is
paid ahead (`credit`), a wallet with an upper bound (`wallet`), moves between two accounts of one
kind (`to_itself`), and a sale of three moves beside an exchange from yen to dollars
(`allocation`), each in English and under a Japanese name. `stock_reservation` has scenarios
written by hand beside it. The books under
[tests/fixtures](https://github.com/i2y/ritsu/tree/main/crates/chobo/tests/fixtures) each show one
diagnostic or a few, in English and in Japanese.
