# What it checks

`chobo check` reads a book and says two things: the diagnostics, where an error stops the book and
a warning does not, and, for a book without errors, what each operation of each transfer can be
refused with. What it says about calling the book comes with the operations that get there, which
it ran in the reference interpreter before saying so; when it cannot find such operations, it says
nothing.

## What each operation can be refused with

The points example keeps each member's points between 0 and a million:

```console
$ chobo check examples/points/points.book
examples/points/points.book: ok
  earn.hold   may be refused: points_cap (points(member) at most 1000000), key_conflict, already_refused
  earn.post   may be refused: key_conflict, already_voided, over_hold, no_such_hold
  earn.void   may be refused: already_posted, no_such_hold
  spend.hold  may be refused: not_enough_points (points(member) at least 0), key_conflict, already_refused
  spend.post  may be refused: key_conflict, already_voided, expired, over_hold, no_such_hold
  spend.void  may be refused: already_posted, expired, no_such_hold
  lapse.do    may be refused: not_enough_points (points(member) at least 0), key_conflict, already_refused
…
```

The reasons that can be there come from the book: the lower bound of an account a move takes from,
and the upper bound of one it puts into; `key_conflict`, when two calls with the same key can differ
in an amount or in another parameter left out of the key; `already_refused`, when a bound can refuse
a call, which spends its key; `same_account`, when a move goes between two accounts of one kind; and
the states of a hold. Each is listed only when the checker finds operations that get there, and
runs them. A
purchase's points never expire, so `earn` is never refused with `expired`; earning only puts in, so
it is never refused with `not_enough_points`.

`--format json` gives those operations, as steps of a scenario with what each answered. Points are
refused with `points_cap` when a member has a million already:

```console
$ chobo check examples/points/points.book --format json
{
  "v": 2,
  "files": [
    {
      "file": "examples/points/points.book",
      "ok": true,
      "diagnostics": [],
      "report": [
        {
          "kind": "earn",
          "op": "hold",
          "refusals": [
            {
              "name": "points_cap",
              "because": "bound",
              "spends_key": true,
              "example": [
                {
                  "op": "hold",
                  "kind": "earn",
                  "args": {
                    "purchase": "purchase-1",
                    "member": "member-2",
                    "pts": 1000000
                  },
                  "result": "done"
                },
                {
                  "op": "post",
                  "kind": "earn",
                  "args": {
                    "purchase": "purchase-1"
                  },
                  "result": "done"
                },
                {
                  "op": "hold",
                  "kind": "earn",
                  "args": {
                    "purchase": "purchase-3",
                    "member": "member-2",
                    "pts": 1
                  },
                  "result": "refused",
                  "reason": "points_cap"
                }
              ],
…
```

The list is what a workflow's task that runs the operation declares as its errors, and what
`chobo api` gives another tool, with the same examples ([Formats](formats.md)). The search behind
it gives an amount 1, 2, 3, 5, 10, 100 or 1000, and to fill an account it takes from, it calls a
transfer that puts into it, three deep at most. A reason only an amount of 0 reaches, or one
deeper than that, is not listed.

## Diagnostics

A diagnostic names its code, the line, and what is wrong. An error stops the book: nothing is run,
built or drawn until it is fixed, and after a line that does not read, nothing else is checked,
since anything after would be a guess. Most errors are about how the book is written:

```text
error[E030]: keys.book:9:10: the transfer `receive` has no `key` line; every transfer needs an idempotency key. List the parameters it happens once for, like `key order`
     9 | transfer receive(slip: string, sku: string, qty: item)
error[E031]: keys.book:12:14: the amount `qty` cannot be part of the key: two calls that differ only in the amount would both go through, as two transfers
    12 |   key order, qty
error[E030]: keys.book:15:7: `slip_no` in the key is not a parameter of `take_back`
    15 |   key slip_no
```

| Codes | What they find |
|---|---|
| E001 to E005 | a line that does not read, a name not declared or declared twice, the number of an account's arguments, a type |
| E010, E011, E014 | a move between units, a number its unit cannot hold, a tax on a unit that is not money |
| E012, E013 | a move from an account to the same account whatever the call, a transfer without a move |
| E020 to E023 | an account of the book without a bound, an outside one with one, bounds no balance fits, a bound without its reason |
| E030, E031 | a transfer without a key, an amount in the key |
| E040, E041 | a hold that does not say how it ends, an expiry out of range |

Four warnings are about what happens when the book is called, and come with the operations that
get there, which the checker ran. A transfer whose moves are in an order that can refuse it, when
the two moves together would leave enough
([tests/fixtures/split.book](https://github.com/i2y/ritsu/blob/main/crates/chobo/tests/fixtures/split.book)):

<div class="cb-term" markdown>

```text
warning[W103]: split.book:13:3: move 1 takes from shop_balance(shop) before move 2 puts into it: when shop_balance(shop) is short at that point, the call is refused with insufficient_balance, even when the two moves together would leave enough
    13 |   move fee from shop_balance(shop) to fees
  the operations that get there:
       1  sale.do(order: order-1, shop: shop-2, price: 1, fee: 1)  refused: insufficient_balance (move 1 takes 1 from shop_balance(shop-2): posted 0, held out 0)
  hint: write the move that puts into shop_balance(shop) first
```

</div>

In a hold, what one move puts in is held coming in, and another move of the same hold cannot take
it, in whatever order they are written:

<div class="cb-term" markdown>

```text
warning[W104]: hold_order.book:14:3: in a hold, what move 1 puts into held(order) is held coming in, and does not count when move 2 takes from held(order): unless held(order) has enough already, the hold is refused with held_short
    14 |   move price from held(order) to shop_payout
  the operations that get there:
       1  settle.hold(order: order-1, price: 1)  refused: held_short (move 2 takes 1 from held(order-1): posted 0, held out 0, held in 1)
  hint: have what it takes in held(order) beforehand, or take it in another transfer after the hold is posted
```

</div>

| Codes | What they find |
|---|---|
| W101 | an account that only ever fills: no move takes out of it |
| W102 | a transfer always refused: nothing puts into what it takes from |
| W103 | a transfer refused for the order of its moves |
| W104 | a move of a hold that counts on what another move of the same hold puts in |
| W105, W106 | something never used, a bound that never matters |
| W107 | with `--diff-base`, an account or a transfer gone since the revision compared with (below) |
| W901 | a key written in the file, in a string or a comment: the diagnostic gives its kind, its prefix and its length, never the key |

Four errors belong to a target, and `chobo build` gives them: a chain of transfers too long for
one TigerBeetle request (E060, at 253); a name longer than PostgreSQL's 63 bytes (E061), which
PostgreSQL would cut short without a word; a book's name the MySQL family's tables and checks
cannot be named after (E062), longer than 51 characters or with a character beyond the Basic
Multilingual Plane, which MySQL would write as `?` with only a warning; and, for the Rust targets,
a name with a character no Rust identifier holds where the name has it (E063), such as a digit that
is not decimal, which the Rust client would write as it is, as a field of a struct:

```text
error[E063]: identifiers.book:11:46: The parameter's name `area_m²` has ² (U+00B2), which no Rust identifier holds
    11 | transfer build(permit: string, site: string, area_m²: sqm)
```

A book is held only to the limits of the target it is built for.

[Diagnostics](codes.md) has all 32 codes, each with when it appears, how to fix it and the smallest
book that shows it, as `chobo explain --all` prints them; `chobo explain <code>` prints one. Every
diagnostic of chobo's on this site is taken from a golden file of chobo's tests, which hold the
diagnostics of each test book, in English and in Japanese, to what the checker prints.

## Changes the data cannot follow

A book's version is not part of what an account or a transfer is, so the balances and the holds a
database has go on under a new version. `chobo check --diff-base <revision>` compares the book with
the one at that git revision, and refuses what they could not follow. The book of
[tests/fixtures/changes.before.book](https://github.com/i2y/ritsu/blob/main/crates/chobo/tests/fixtures/changes.before.book)
keeps stock at 0 or more, and keys a delivery on its slip and its SKU:

```book
book changes v1
…
account stock(sku: string) : item
  at least 0 refused as out_of_stock
…
transfer receive(slip: string, sku: string, qty: item)
  key slip, sku
  move qty from supplier to stock(sku)
transfer reserve(order: string, sku: string, qty: item)
  key order, sku
  pending expires after 30 minutes
```

Its next version keeps 3 on the shelf and at most 1000, keys a delivery on its slip alone, holds an
order's stock for an hour, and drops the shelf of returns with the two transfers that used it:

```book
book changes v2
…
account stock(sku: string) : item
  at least 3 refused as out_of_stock
  at most 1000 refused as no_room
…
transfer receive(slip: string, sku: string, qty: item)
  key slip
…
  pending expires after 1 hour
```

<div class="cb-term" markdown>

```text
warning[W107]: changes.book:1:1: the account `returns_shelf` of HEAD is gone: its balances stay in the database
     1 | book changes v2
warning[W107]: changes.book:1:1: the transfer `take_back` of HEAD is gone: a retry of it still in flight can no longer be made
     1 | book changes v2
warning[W107]: changes.book:1:1: the transfer `restock` of HEAD is gone: a retry of it still in flight can no longer be made
     1 | book changes v2
error[E050]: changes.book:5:9: the account `stock` differs from HEAD in its lower bound (0 → 3), its upper bound (none → 1000): the accounts that already exist keep the old definition, so chobo cannot make this change
     5 | account stock(sku: string) : item
  hint: declare an account under a new name, and write a transfer that moves the balances over
error[E051]: changes.book:11:10: the transfer `receive` differs from HEAD in its key: a retry in flight would be refused with key_conflict
    11 | transfer receive(slip: string, sku: string, qty: item)
  hint: declare the new form under a new name, and keep the old one until every hold it made has ended
error[E051]: changes.book:14:10: the transfer `reserve` differs from HEAD in how its holds end: a retry in flight would be refused with key_conflict, and a hold still held could not be posted
    14 | transfer reserve(order: string, sku: string, qty: item)
  hint: declare the new form under a new name, and keep the old one until every hold it made has ended
```

</div>

The databases hold the same line when the book is called. PostgreSQL and the MySQL family keep
the bounds an account was made with in its row, and stop a call whose book says other bounds with
the error `CB001`.
TigerBeetle's client finds the account, or the opening transfer that keeps its bound, already there
with another definition, and fails. To change a bound, declare an account kind under a new name and write a transfer that moves
the balance over; a limit that will change is an account of its own from the start.

## Across the languages

In a project with workflows and rules, `ritsu check` checks the book where a workflow calls it: the
amounts a rule hands to a transfer, the reasons the transfer can be refused with against the errors
the task handles, and a hold against the wait before it is posted.
[With the other languages](together.md) shows each.
