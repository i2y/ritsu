# The chobo language

A book (`.book`) holds three things: the units things are counted in, the accounts they are kept
in with the bounds each account keeps, and the kinds of transfer that move them from account to
account. This page is the whole language. [formats.md](formats.md) has the JSON chobo reads and
writes, [targets.md](targets.md) what `chobo build` writes for PostgreSQL and TigerBeetle, and
[codes.md](codes.md) every diagnostic.

## A book

The inventory example ([examples/inventory/inventory.book](https://github.com/i2y/chobo/blob/main/examples/inventory/inventory.book)):

```book
book inventory v1
description "Stock per SKU. A delivery adds to it, an order holds what it takes for 30 minutes, shipping posts the hold, cancelling voids it, and a return puts the goods back on the shelves"

unit pcs

account stock(sku: string) : pcs
  description "what is on the shelves"
  at least 0 refused as out_of_stock
account suppliers : pcs outside
account customers : pcs outside

transfer receive(delivery: string, sku: string, qty: pcs)
  key delivery, sku
  move qty from suppliers to stock(sku)

transfer reserve(order: string, sku: string, qty: pcs)
  description "posted when the order ships, voided when it is cancelled"
  key order, sku
  pending expires after 30 minutes
  move qty from stock(sku) to customers

transfer take_back(return_id: string, sku: string, qty: pcs)
  description "what a customer sent back, on the shelves again"
  key return_id, sku
  move qty from customers to stock(sku)
```

The first line names the book and its version. Indentation makes the blocks, and `#` starts a
comment that runs to the end of the line. The keywords are English; the names (of the book, the
units, the accounts, the transfers, their parameters and the reasons of refusal) can be in any
language, and the examples have a Japanese twin beside each book (`inventory.ja.book`).

## Units

```book
unit pcs
unit USD scale 2
```

An amount is a whole number of the smallest step of its unit: one piece, one cent. `scale` says
how many decimal places the literals of the book (bounds and amounts) are written with, and
`chobo doc` shows: in `USD scale 2`, `12.50` is 1250 cents. An amount is from 0 to 2⁶³ − 1
(what PostgreSQL's `bigint` holds), and a bound from −(2⁶³ − 1) to 2⁶³ − 1.

The account a move takes from, the account it puts into, and its amount are in one unit (E010).
chobo converts nothing: an exchange is two moves, one in each unit, each through an account
outside the book, with both amounts worked out by the caller.

```book
unit 円 incl_tax
```

A unit of money may end with whether its amounts are with tax or without: `incl_tax` or
`excl_tax`. Nothing in the book changes with it; it is for amounts handed over from a rule or a
workflow, which are to say the same. The book hands its units to ritsu's other languages as the
units rulec writes:

| Unit | As the other languages see it |
|---|---|
| a currency's name (`円`, `JPY`, an ISO 4217 code, `銭`, `USDc`) with scale 0 | that money: `money[円]`, or with its tax `money[円, incl_tax]` |
| a currency's name other than `円` and `JPY`, with scale 2 | its hundredth: `unit USD scale 2` is `money[USDc]` |
| a unit of mass, length, area, volume or time (`g`, `kg`, `L`, `h`) with scale 0 | that unit: `mass[kg]` |
| anything else (`pcs`, `seats`, `pt`, yen with scale 2) | a count with its name and nothing else, the same only as itself |

Only the money of the first two rows takes a tax (E014); the hundredth of a yen is `銭`.

## Accounts

```book
account points(member: string) : pt
  at least 0 refused as not_enough_points
  at most 1000000 refused as points_cap
account issued : pt outside
```

- **Parameters.** An account kind is split by its parameters, which are strings: one account for
  each member, each SKU, each day. An account (`points("m-1")`) is there from its first use, with
  a balance of 0.
- **The balance** is what came in, less what went out. An account outside the book (`outside`)
  stands for the world the book trades with, a supplier, the buyers, a bank: it has no bounds,
  and its balance goes below 0 as things come from it.
- **Bounds.** An account of the book (one that is not `outside`) has `at least`, `at most` or
  both (E020), each with the reason a transfer that would break it is refused with
  (`refused as`, E023). That name is what a caller gets back, and the name of the error a
  dandori task declares. An upper bound is 0 or more (E022); a lower bound may be above 0
  (keep 3 on the shelf) or below it (credit down to −50000).
- A bound is a constant of the account kind. A limit that differs from account to account (a
  member's credit line) is an account of its own, which a transfer fills.

A bound is the only condition a book can write, and that is on purpose: a bound looks at one
account, so the write that changes the account can check it, with nothing else to read in
between. A rule over two accounts, *a refund never exceeds the sale*, becomes the bound of an
account of its own: what is left to refund on each order, which the sale fills and a refund
takes from, and which never goes below 0
([examples/refunds](https://github.com/i2y/chobo/blob/main/examples/refunds/refunds.book)).

## Transfers

```book
transfer sale(order: string, shop: string, to_shop: USD, fee: USD)
  description "to_shop and fee add up to what the buyer paid; the caller works out the fee"
  key order
  move to_shop from buyers to shop_balance(shop)
  move fee from buyers to fees
```

- **Parameters** are strings or amounts of a unit.
- **`key`** names the parameters that make a call once only (E030): the same key with the same
  arguments does nothing the second time, and with other arguments is refused. An amount cannot
  be part of the key (E031).
- **`pending`** makes the transfer hold first: `pending expires after <n> seconds|minutes|hours|days`
  (from 1 second to 2³² − 1 seconds, E041) or `pending never expires` (E040). Without it, the
  transfer moves at once.
- **`move <amount> from <account> to <account>`**: the amount is a parameter or a literal, and
  the accounts take the transfer's parameters (or string literals) as their arguments, by
  position. A transfer has one move or more (E013), made in the order written, all or none.

A transfer computes nothing: an amount is given, not worked out from a balance or another amount.
A fee, a rate or a rounding belongs to the caller, or to a rulec rule the caller asks.

## Operations and what they answer

| Operation | On | What it does |
|---|---|---|
| `do` | a transfer that moves at once | makes the moves |
| `hold` | a transfer that holds first | holds what the moves would move |
| `post` | the same | makes the hold final: all of it, or the amounts given (the rest goes back) |
| `void` | the same | lets the hold go |

Each answers one of:

- `done`: done now.
- `done_before`: the same call, with the same key and the same contents, was done before;
  nothing happens.
- `refused`, with a reason: nothing happens.

A refusal is an answer of the business (there is no stock), not a failure, so it comes back as a
result, not an exception. A failure is something else: a database that cannot be reached,
arguments of the wrong type, an amount out of range, or a balance that would leave the range of a
64-bit integer. A call that failed may be retried with the same arguments; its key keeps it from
moving twice.

Two reads go with them: the `balance` of an account (what is posted, held going out, and held
coming in), and the `status` of a hold (`held`, `posted`, `voided` or `expired`, or none).

## How a bound is checked

The moves are made one at a time, in the order written, and each is checked against the
balances the moves before it left:

- a move of `a` out of an account with `at least L` goes through only when
  `posted − held_out − a ≥ L`: what is held going out counts, what is held coming in does not;
- a move of `a` into an account with `at most U` goes through only when
  `posted + held_in + a ≤ U`: what is held coming in counts, what is held going out does not.

A hold is checked the same way when it is made, and adds to `held_out` and `held_in`; posting it
moves what it holds from held to posted, and voiding or expiry lets it go. Neither is checked
again: the hold was checked for the worst case when it was made, so a post never breaks a bound.

When a move is refused, the moves before it are undone, and the answer is the reason of the first
bound refused. Moves in the order written can refuse what the moves together would allow: a
transfer that takes from an account before putting into it is refused when the account is short
at that point (W103), and in a hold, what one move holds coming in cannot be taken by another
(W104).

## Keys

- Keys are kept apart by transfer kind and operation: `post` and `void` take the key of the hold.
- The contents compared are every argument for `do` and `hold`, the amount of each move for
  `post` (all of it counts as the amount held), and nothing for `void`.
- The same key with the same contents answers `done_before`; with other contents, `key_conflict`.
- A key a bound refused stays refused: the next call with it answers `already_refused`, whatever
  it carries, even once the balance is enough. To try again, use another key (with the attempt in
  it).
- A key refused for another reason (`same_account`, `no_such_hold`, the state of a hold) is not
  spent.
- A hold's key cannot be used again after the hold has ended: holding again with it answers
  `done_before`, and holds nothing.

## The life of a hold

| The hold | `post` | `void` | it expires |
|---|---|---|---|
| held | posted (refused with `expired` from its expiry on) | voided (likewise) | expired |
| posted | `done_before` with the same amounts, `key_conflict` with others | `already_posted` | — |
| voided | `already_voided` | `done_before` | — |
| expired | `expired` | `expired` | — |
| no hold with the key | `no_such_hold` | `no_such_hold` | — |

A post with amounts posts each move for what its amount parameter says now, and lets the rest
go; a move with a literal amount is posted in full. A post of more than a move holds is refused
with `over_hold`. A hold expires when its expiry comes, outside the caller's hands; one that
`never expires` is ended by the caller alone. `chobo api` gives the life of a hold as a state
machine, in the shape dandori reads a rulec rule's in, for a workflow's checker to follow
(dandori does not read it yet).

## The reasons chobo gives

| Reason | When | Does it spend the key |
|---|---|---|
| a bound's own (`out_of_stock`) | a move would break the bound | yes: `already_refused` from then on |
| `key_conflict` | the same key with other contents | no: the call before stands |
| `already_refused` | a key a bound refused, again | — |
| `same_account` | a move whose two accounts are the same (the same arguments) | no |
| `no_such_hold` | a post or a void of a hold that is not there | no |
| `already_posted` | a void of a posted hold | no |
| `already_voided` | a post of a voided hold | no |
| `expired` | a post or a void of an expired hold | no |
| `over_hold` | a post of more than the hold holds | no |

`chobo check` lists, for each operation of each transfer, the reasons it can be refused with,
each found by a short list of operations that the reference interpreter runs to it; `--format
json`, `chobo api` and `chobo doc` show those operations.

## Scenarios

A scenario is a list of steps: operations, `pass <duration>` (the clock of the reference
interpreter moves on, and the holds whose expiry it reaches expire), and `together` (the
operations of two callers or more at the same time, eight at most). `chobo run` runs one, and
for `together` gives every way it can come out, trying every order the callers' operations can
interleave in. `chobo scenarios` writes them for a book: each bound just before, at and past
it, each key used twice, every way a hold ends, a refusal partway through the moves, and two
callers after the last of something. [formats.md](formats.md) has their JSON.

## Versions

A book's version (`v1`) is not part of what an account or a transfer is: a new version goes on
with the balances and holds the database has. So `chobo check --diff-base <revision>` compares
the book with the one at that git revision, and refuses what the data could not follow: a change
to an account kind's unit, scale, parameters, bounds or whether it is outside (E050), and to a
transfer's parameters, key, hold or moves (E051); an account kind or a transfer that is gone, or
a book with another name, is a warning (W107). To change a bound, declare a new account kind and
write a transfer that moves the balance.

## Names and keywords

A name starts with a letter or `_`, and goes on with letters, digits and `_`. Names are compared
as they are written, without Unicode normalization; a name with a combining mark (a dakuten or
an accent written as a character of its own) is refused (E001), to be written precomposed.

| Where | Keywords |
|---|---|
| at the start of a line | `book` `description` `unit` `account` `transfer` `key` `pending` `move` |
| within a line | `scale` `incl_tax` `excl_tax` `outside` `at least` `at most` `refused as` `expires after` `never expires` `from` `to` |
| as a type | `string` |
| after `expires after <n>`, and only there | `second` `seconds` `minute` `minutes` `hour` `hours` `day` `days` |

No name may be a word of the first three rows. The words of durations are keywords only after
`expires after <n>`, so a parameter can be called `day`.

## Diagnostics

Every diagnostic has a code, the line it is about, and, when it only shows when the book is
called, the operations that get there, which the checker ran in the reference interpreter before
saying so. [codes.md](codes.md) has all 29 codes, as `chobo explain --all` prints them, and
`chobo explain <code>` prints one.
