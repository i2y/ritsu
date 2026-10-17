# Money and units

Every amount in a book is a whole number of the smallest step of its unit, and every move is in one
unit. A unit is money, a quantity such as a kilogram, or a count with a name, and the other
languages of ritsu see it as one of their own types: a book of US dollars takes the amounts a rule
gives in cents, and a book that keeps money with tax takes no amount without it.

## The smallest step

```book
unit pcs
unit USD scale 2
```

`unit pcs` counts pieces, one at a time. `unit USD scale 2` counts cents, and says that the
literals of the book, its bounds and the amounts its moves write, have two decimal places: a bound
of `12.50` is 1250 cents. `chobo doc`, and the bounds `chobo check` names, show amounts so; a
scenario, what `chobo run` prints and what a client is called with take the whole number. An
amount is from 0 to 2⁶³ − 1, what a `bigint` of PostgreSQL or MySQL holds, and a bound from
−(2⁶³ − 1) to 2⁶³ − 1. A number its unit cannot hold stops the book:

```text
error[E011]: numbers.book:9:11: `10.505` has 3 decimal places, and the unit `USD` takes 2
     9 |   at most 10.505 refused as over_limit
error[E011]: numbers.book:11:12: `-9223372036854775808` is out of range: from −(2⁶³ − 1) to 2⁶³ − 1, counted in the unit's smallest step
    11 |   at least -9223372036854775808 refused as over_allowance
error[E011]: numbers.book:17:8: the amount of a move cannot be negative; swap `from` and `to` to move the other way
    17 |   move -1 from stock(sku) to customer
```

## One unit a move

A move takes from an account and puts into another of the same unit, for an amount of that unit.
chobo converts nothing, so a move between units is an error, which names the unit of each side:

```text
error[E010]: units.book:12:3: the units of this move do not match: the account it moves from, `stock`, is in `item`, the one it moves to, `customer`, in `USD`, and the amount `qty` in `item`; to exchange one unit for another, write two moves, each through an outside account of its unit
    12 |   move qty from stock(sku) to customer
```

An exchange is two moves, one in each unit, each through an account outside the book, with both
amounts worked out by the caller. Units are told apart by their names alone: to keep the seats of a
hall apart from the pieces in a warehouse, declare `unit seats` beside `unit pcs`.

## Money with tax or without

A unit named for a currency is money, and it may end with whether its amounts are with tax or
without, `incl_tax` or `excl_tax`:

```book
unit USD scale 2 incl_tax
```

Inside the book nothing changes: amounts are counted and bounded the same way, and the code a build
writes is the same with it or without it. It is for where an amount comes into the
book from a rule or a workflow, which must then say the same: a unit with tax takes amounts with
tax, a unit without tax amounts without, and a unit that says neither amounts that say neither, so
that one account never ends up holding both. A unit that is not money takes neither:

```text
error[E014]: tax_kinds.book:4:11: the unit `item` is not money, so it takes no `incl_tax`
     4 | unit item incl_tax
  hint: take `incl_tax` off: only a unit named for a currency says whether it is with tax or without (yen and JPY with scale 0, any other currency with scale 0 or 2)
```

## As the other languages see a unit

A book hands its units to ritsu's other languages as the units rulec writes:

| The unit | As rulec and dandori see it |
|---|---|
| a currency's name (`円`, `JPY`, an ISO 4217 code, `銭`, `USDc`) with scale 0 | that money: `money[JPY]`, or with its tax `money[JPY, incl_tax]` |
| a currency's name other than `円` and `JPY`, with scale 2 | its hundredth: `unit USD scale 2` is `money[USDc]` |
| a unit of mass, length, area, volume or time (`g`, `kg`, `L`, `h`) with scale 0 | that unit: `mass[kg]` |
| anything else (`pcs`, `seats`, `pt`, yen with scale 2) | a count with its name, the same only as itself |

The hundredth of a yen is `銭`, so yen with scale 2 is a count of chobo's alone. `pcs` and `seats`
are never the same unit, and `kg` is the kilogram the rules know.

Where a dandori workflow runs an operation of the book, the task's amount is the book's unit as
that table says: the refunds example's `amount` is `money[USDc]`, and the inventory example's
`qty`, a count, is an `int`. A task that gives the book another type is dandori's E016, and a
rule's output of another unit, or with another tax, given to that task is dandori's E003; both
come from `ritsu check`, which reads the book, the rule and the workflow together. `ritsu check`
also holds the amounts a rule can return to the 0 or more that chobo takes (ritsu's E203), and the
reasons the transfer can be refused with, for those amounts, to the errors the task handles
([With the other languages](together.md)).

## What a book does not compute

A transfer is given its amounts. It does not read a balance to work one out, and it adds nothing
up: a fee, a rate, a rounding is the caller's, or a rulec rule's that the caller asks. *The fee is
10% of the price* is not a bound of an account; the marketplace example takes the shop's part and
the fee as two amounts that add up to what the buyer paid, worked out before the call. What chobo
decides is whether a transfer goes through, and with which bound's reason it is refused when it
does not, so that every reason can be listed before anything runs.
