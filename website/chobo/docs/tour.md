# Write a book

A `.book` file is one book: the units things are counted in, the accounts they are kept in with
the bounds each account keeps, and the kinds of transfer that move them from account to account.
This page walks through the marketplace example,
[examples/marketplace/marketplace.book](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/marketplace/marketplace.book):
a marketplace keeps a balance for each shop, a sale splits what the buyer paid between the shop and
the marketplace's fee, a refund takes both back, and a payout to the shop's bank is held until the
bank confirms it. Two passages of the points example,
[examples/points/points.book](https://github.com/i2y/ritsu/blob/main/crates/chobo/examples/points/points.book),
show an upper bound and a hold that expires. [Reference](reference.md) has the whole language on one
page.

## The first lines

```book
# A sale splits what the buyer paid between the shop and the marketplace's fee, in amounts the caller works out.
book marketplace v1
description "The marketplace keeps a balance for each shop. A sale splits what the buyer paid between the shop's balance and the marketplace's fee, in amounts the caller works out (a rule decides the fee). A payout is held until the bank confirms it, and a refund takes both parts back"
```

- `book marketplace v1` names the book and its version. The version is not part of what an
  account or a transfer is, so `v2` goes on with the balances and holds the database already has;
  `chobo check --diff-base` says what a new version cannot change
  ([What it checks](checks.md#changes-the-data-cannot-follow)).
- `#` starts a comment, to the end of the line, and indentation makes the blocks.
- `description` says what the book is for. It goes on the page for people `chobo doc` writes, and
  into what `chobo api` says of the book; an account and a transfer can have one too.

The keywords are English, and the names (of the book, its units, accounts, transfers, parameters
and reasons) can be in any language. Each example has a Japanese version beside it,
`marketplace.ja.book`, with every name in Japanese:

```book
account 売上金(店: string) : 円
  at least 0 refused as 売上金不足
```

## Units

```book
unit USD scale 2
```

An amount is a whole number of the smallest step of its unit: here a cent. `scale 2` says that the
literals of the book, its bounds and the amounts it writes, have two decimal places, and that
`chobo doc` shows them so: `12.50` is 1250 cents. A unit that is not money, `unit pcs`, counts
pieces. One move takes from an account and puts into another of the same unit, for an amount of
that unit (E010); chobo converts nothing. [Money and units](units.md) says more, and how money says
whether it is with tax or without.

## Accounts and their bounds

```book
account shop_balance(shop: string) : USD
  description "what the marketplace owes the shop"
  at least 0 refused as insufficient_balance
account buyers : USD outside
account fees : USD outside
account bank : USD outside
```

- **One account for each shop.** An account kind is split by its parameters, which are strings:
  `shop_balance("shop-2")` is the balance of one shop. An account is there from its first use, with
  a balance of 0.
- **The balance** is what came in, less what went out. An account `outside` the book stands for
  the world the book trades with: here the buyers, the fees the marketplace keeps, and the bank. It
  has no bounds, and its balance goes below 0 as things come from it.
- **A bound and its reason.** `at least 0 refused as insufficient_balance`: no transfer takes the
  balance below 0, and one that would is refused with `insufficient_balance`. That name is what a
  caller gets back, and the name of the error a workflow's task declares. An account of the book
  has a bound (E020), an outside one has none (E021), and every bound names its reason (E023).

The points example has an upper bound too. A member's points never go below 0 or past a million:

```book
account points(member: string) : pt
  at least 0 refused as not_enough_points
  at most 1000000 refused as points_cap
```

A lower bound may be above 0 (keep 3 on the shelf) or below it (credit down to −50000); an upper
bound is 0 or more (E022). A bound is a constant of the account kind. A limit that differs from
account to account, a member's credit line, is an account of its own, which a transfer fills.

A bound is the only condition a book can write, and that is on purpose: it looks at one account,
so the write that changes the account can check it, with nothing else to read in between. A
condition over two accounts is written as the bound of an account of its own, as the refunds
example writes *a refund never exceeds the sale* ([Design](design.md)).

## Transfers

```book
transfer sale(order: string, shop: string, to_shop: USD, fee: USD)
  description "to_shop and fee add up to what the buyer paid; the caller works out the fee"
  key order
  move to_shop from buyers to shop_balance(shop)
  move fee from buyers to fees
```

- **Parameters** are strings or amounts of a unit.
- **`key order`**: a sale happens once for each order. The same call again, with the same
  arguments, does nothing and answers `done_before`, so a retry never moves twice. A call with
  the same order and other arguments is refused with `key_conflict`. An amount cannot be part of
  the key (E031): two calls that differ only in the amount would both go through.
- **`move <amount> from <account> to <account>`**, one line a move. The amount is a parameter or
  a literal, and an account takes the transfer's parameters (or string literals) as its arguments,
  by position. The moves are made in the order they are written, and all of them go through or
  none does.

A transfer computes nothing: the shop's part and the fee are given, worked out by the caller or by
a rulec rule the caller asks. A refund gives both back:

```book
transfer refund(refund_id: string, shop: string, to_shop: USD, fee: USD)
  description "gives back the fee and the shop's part, both or neither"
  key refund_id
  move fee from fees to buyers
  move to_shop from shop_balance(shop) to buyers
```

The moves are checked one at a time, against the balances the moves before them left. When the
shop's balance is short, the second move is refused with `insufficient_balance`, the first is
undone, and nothing has moved. Because each move is checked where it stands, the order can matter:
a transfer that takes from an account before another of its moves puts into it can be refused
even when the two together would leave enough, which the checker says (W103,
[What it checks](checks.md#diagnostics)).

## Holds

```book
transfer payout(payout_id: string, shop: string, amount: USD)
  description "held when the bank transfer starts: posted when the bank confirms it, voided when it fails"
  key payout_id
  pending never expires
  move amount from shop_balance(shop) to bank
```

`pending` makes a transfer hold first. Its operations are `hold`, which holds what the moves would
move; `post`, which makes the hold final, for all of it or for the amounts given (the rest goes
back); and `void`, which lets it go. A payout never expires: the bank's answer ends it, posted or
voided. A hold can also end by itself, as a checkout's spending of points does after 15 minutes:

```book
transfer spend(checkout: string, member: string, pts: pt)
  description "posted when the payment goes through, voided when it fails"
  key checkout
  pending expires after 15 minutes
  move pts from points(member) to redeemed
```

A hold is checked when it is made, for the worst case: what is held going out counts against a
lower bound, and what is held coming in counts against an upper one, while neither counts the
other way. So a post never breaks a bound, and is never refused for one. Once a hold has ended,
holding again with its key answers `done_before` and holds nothing; a new hold takes a new key.

| The hold | `post` | `void` | it expires |
|---|---|---|---|
| held | posted (refused with `expired` from its expiry on) | voided (likewise) | expired |
| posted | `done_before` with the same amounts, `key_conflict` with others | `already_posted` | — |
| voided | `already_voided` | `done_before` | — |
| expired | `expired` | `expired` | — |
| no hold with the key | `no_such_hold` | `no_such_hold` | — |

## What each operation can answer

A transfer that moves at once has one operation, `do`; one that holds first has `hold`, `post` and
`void`. Each answers `done`, `done_before`, or `refused` with a reason, and a refusal changes
nothing. `chobo check` lists the reasons for the whole book:

```console
$ chobo check examples/marketplace/marketplace.book
examples/marketplace/marketplace.book: ok
  sale.do      may be refused: key_conflict
  refund.do    may be refused: insufficient_balance (shop_balance(shop) at least 0.00), key_conflict, already_refused
  payout.hold  may be refused: insufficient_balance (shop_balance(shop) at least 0.00), key_conflict, already_refused
  payout.post  may be refused: key_conflict, already_voided, over_hold, no_such_hold
  payout.void  may be refused: already_posted, no_such_hold
  key: sale once per order; a second call that differs only in shop, to_shop or fee is refused with key_conflict
  key: refund once per refund_id; a second call that differs only in shop, to_shop or fee is refused with key_conflict
  key: payout once per payout_id; a second call that differs only in shop or amount is refused with key_conflict; holding again with the same key after the hold has ended answers done_before and holds nothing
```

A payout that never expires is never refused with `expired`, and a sale, which only puts in, is
never refused by a bound: each reason is listed because the checker found operations that get
there. `already_refused` is the answer to a key a bound refused, called again: the key stays
spent, so a retry answers the same, and trying again takes a new key. [What it checks](checks.md)
goes on from here, and [Run scenarios](scenarios.md) runs the book.
