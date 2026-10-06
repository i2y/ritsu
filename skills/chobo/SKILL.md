---
name: chobo
description: Write, check and build chobo books (`.book` files), a small language for the things counted and moved between accounts (stock, money, points, seats), whose only conditions are the lower and upper bounds of accounts, each kept in one write on PostgreSQL or TigerBeetle. Use when stock has to be reserved without selling more than there is, a wallet or points kept from going below 0, refunds kept within the sale, a payment split between parties all or none, or a hold that must be posted or voided; when a chobo diagnostic (E001-E061, W101-W107, W901) has to be fixed; when a book has to be shown to the people who keep the accounts; or when a book has to be built for PostgreSQL or TigerBeetle and its client (TypeScript, Python or Go) called.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run chobo as `ritsu chobo <command>`, or as `chobo <command>` through a link to ritsu named for it. The code it builds needs PostgreSQL, or TigerBeetle with its official client 0.17.9, in the program that calls it.
license: MIT OR Apache-2.0
---

## When this applies

The job is a **quantity that moves between accounts and must stay within bounds**: stock that
orders reserve and shipments take, a wallet that payments draw on, points earned and spent,
seats of a day, what is left to refund on an order, a shop's balance that sales fill and payouts
empty. Two orders after the last item at the same moment, a refund retried twice, a hold nobody
ended: chobo writes the code where those go wrong, from a book a person can read.

It does not apply to computing amounts: a fee, a rate, a rounding. A book moves amounts it is
given; the caller works them out, or a rulec rule does (rulec has a skill of its own). Nor to the
steps of a process (wait, retry, ask a person): that is a workflow, which dandori writes and
which calls the book's transfers as tasks. Nor to a general ledger: a balance in chobo is what
came in less what went out, not debits and credits.

The files bundled with this skill are listed in §8. Read them when you need them, not all up
front.

---

# Working with chobo

Your part is to write the `.book`, get it past `ritsu chobo check`, and build it. Two things stay with
people:

- **the numbers and the names of the business**: the bounds, the reasons a refusal gives, what
  makes a call once only, how long a hold lasts (§4 says when to ask);
- **the code around the book**: where the amounts come from, who posts and voids a hold, the
  connection to the database.

Run every command here as `ritsu chobo <command>` (through a link to ritsu named chobo,
`chobo <command>` is the same). Everything is reachable from the command line: `ritsu chobo --help`
lists the commands, and `ritsu chobo check --format json` gives the diagnostics and the report as
data (§6).

## 1. The loop

1. **Find the quantities and the accounts.** What is counted (pieces, cents, points, seats),
   where it is kept (one account for each SKU, each member, each order), and where it comes from
   and goes to outside the book (suppliers, customers, a bank: `outside` accounts).
2. **Turn every condition into a bound of one account** (§3). A condition over two accounts
   cannot be written; add the account that holds what the condition is about.
3. **Write the book** (§2): the units, the accounts with their bounds, then the transfers with
   their keys, holds and moves.
4. **Check it:** `ritsu chobo check <file.book>` until it prints `<file.book>: ok` with no warning;
   fix each diagnostic as §5 says (`ritsu chobo explain <code>` for one code).
   Under it, the checker lists what each operation can be refused with; each reason is there
   because the checker found operations that get there, and `--format json` gives them.
5. **Watch it run:** `ritsu chobo scenarios <file.book> --out <dir>` writes scenarios (`001.json`, …)
   that take every bound before, at and past it, every key twice, every way a hold ends, and two
   callers after the last of something; `ritsu chobo run <file.book> --scenario <dir>/001.json` plays
   one in the reference interpreter, and with two callers at once gives every way it can come
   out.
6. **Show it to a person:** `ritsu chobo doc <file.book> > <file>.md` writes the page for people, for
   whoever keeps the accounts: the bounds, a chart of the moves, what each operation can be refused with, the
   life of a hold, and every scenario with the balances after each step. `--format html` writes
   one page whose scenarios can be stepped through; `--lang ja` writes it in Japanese.
7. **Build it:** `ritsu chobo build <file.book> --target <target> --out <dir>` (§7). A build stops when
   a target cannot take the book (E060, E061).
8. **Call it:** a refusal is an answer, not an exception: handle each reason the check listed. A
   retry with the same arguments is safe; the key makes it `done_before`. On PostgreSQL, call
   `expire()` from a job, one at a time.
9. **In a project with workflows and rules**, `ritsu check` checks where a workflow calls the
   book: an amount a rule gives a transfer stays within 0 to 2⁶³ − 1 (0 is taken), every reason
   the transfer can be refused with is an error the task handles, and a hold has not always
   expired by the time it is posted or voided (ritsu's E203, E204 and E206; the ritsu skill).

## 2. The language on one page

The inventory example (`examples/inventory/inventory.book`):

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
```

Indentation makes the blocks; `#` starts a comment. Keywords are English; names can be in any
language.

```text
book <name> v<N>                       then description "<text>"
unit <name> [scale <digits>] [incl_tax|excl_tax]    an amount is a whole number of the smallest step: scale 2 counts cents; money may say with tax or without
account <name>[(<param>: string, …)] : <unit> [outside]
  description "<text>"
  at least <n> refused as <reason>     a bound, and the reason a call that would break it gets
  at most <n> refused as <reason>      an account of the book needs one of them; an outside one has none
transfer <name>(<param>: string | <unit>, …)
  description "<text>"
  key <param>, …                       what makes a call once only; never an amount
  pending expires after <n> seconds|minutes|hours|days
  pending never expires                holds first, until posted or voided (and expiry, if any)
  move <amount> from <account> to <account>     one or more; in order; all or none
```

An account takes its arguments by position (`stock(sku)`), from the transfer's parameters or
string literals. The operations are `do` (moves at once), or `hold`, `post` (all, or the amounts
given) and `void` for a transfer that holds first. Each answers `done`, `done_before` (the same
key and contents again: nothing happens) or `refused` with a reason. [reference.md](reference.md)
has the whole language.

## 3. Turning a condition into a bound

| The condition | The account it becomes |
|---|---|
| no more is sold than is in stock | `stock(sku)` `at least 0` |
| a refund never exceeds the sale | `refundable(order)` `at least 0`: the sale puts in, a refund takes out |
| at most 120 seats a day | `free_seats(day)` `at least 0`: opening the day puts 120 in |
| the stock of all SKUs fits the warehouse | `warehouse_room` `at least 0`: a delivery takes from it in a second move |
| at most 3 refunds a month | `refunds_left(month)` `at least 0`, in a unit of times |
| a member's credit line, which differs by member | a `credit(member)` account a transfer fills, not a bound per member |
| a wallet holds at most 1000000 | `wallet(member)` `at most 1000000` |
| the fee is 10% of the price | not a bound: the caller (or a rulec rule) works out the fee and passes it |

A bound is a constant of the account kind, and only ever one account's; that is what lets every
database keep it in one write. Moves are checked one at a time in the order written: put into an
account before taking from it in the same transfer (W103), and do not take, within one hold,
what another move of the same hold puts in (W104).

## 4. What stays with a person

Ask instead of guessing:

- **The bounds and the names of their reasons.** `out_of_stock`, `返金超過`: the name is what a
  caller gets back, and what a workflow's task declares as its error.
- **What makes a call once only** (`key`). Usually the business's own number: an order and a
  SKU, a refund request, a payout. An amount never goes in it. A parameter outside the key makes
  a call that differs only in it a `key_conflict`.
- **How a hold ends**: how long it lasts before it expires, or that it never expires and someone
  always posts or voids it; and who does.
- **Which accounts are outside the book** (they go below 0 and have no bounds) and which are kept
  by it.
- **Who works out the amounts** of a transfer with more than one move (the shop's part and the
  fee), since chobo computes nothing.

Ask with the scenario the checker or `ritsu chobo run` gives, in the reader's terms: "Two refunds of
20.00 come at once for a sale of 30.00; one is held and the other refused with
`refund_exceeds_sale`. Is that the rule, or should a refund wait?" The page of `ritsu chobo doc` shows
it step by step, when a person would rather see it.

## 5. From a diagnostic to a fix

A diagnostic names the code, the place and what is wrong, and, when it shows only when the book
is called, the operations that get there:

```text
warning[W103]: split.book:13:3: move 1 takes from shop_balance(shop) before move 2 puts into it: when shop_balance(shop) is short at that point, the call is refused with insufficient_balance, even when the two moves together would leave enough
    13 |   move fee from shop_balance(shop) to fees
  the operations that get there:
       1  sale.do(order: order-1, shop: shop-2, price: 1, fee: 1)  refused: insufficient_balance (move 1 takes 1 from shop_balance(shop-2): posted 0, held out 0)
  hint: write the move that puts into shop_balance(shop) first
```

| Code | What it finds | The usual fix |
|---|---|---|
| E001 to E005 | syntax, a name not declared, a name declared twice, the number of an account's arguments, a type | what the message says |
| E010 | a move whose accounts and amount are in different units | one unit a move; an exchange is two moves through outside accounts |
| E011 | a number that does not fit its unit (decimals past `scale`, a negative amount, out of range) | write it in the unit's decimal places |
| E012 | a move from an account to the same account | another account, or other arguments |
| E013 | a transfer without `move` | write its moves |
| E020 to E023 | an account of the book without a bound, an outside one with one, bounds no balance fits, a bound without `refused as` or with a reason chobo keeps | give each account of the book a bound with its reason |
| E030, E031 | a transfer without `key`, or an amount in the key | key it on what makes the call once only |
| E040, E041 | `pending` without its end, an expiry out of range | `pending expires after <n> <unit>` or `pending never expires` |
| E050, E051, W107 | with `--diff-base`, a change the balances in a database cannot follow | a new account kind or transfer, and a transfer that moves the balance |
| E060 | one operation's chain is too long for one TigerBeetle request | split the transfer into kinds with fewer moves |
| E061 | a name past PostgreSQL's 63 bytes | a shorter name |
| W101 | an account that only ever fills | a transfer that takes from it, or `outside` |
| W102 | a transfer always refused: nothing fills what it takes from | a transfer that fills it |
| W103 | a transfer refused because of the order of its moves | the move that puts in first (the one that takes out first, for an upper bound) |
| W104 | a hold that counts on what another of its moves puts in | have it there beforehand, or a second transfer after the post |
| W105, W106 | something never used, a bound that never matters | remove it, or write what uses it |
| W901 | a key written in the file, in a string or a comment (the kind, its prefix and its length; never the key) | take it out and read it from where the code runs; revoke it first if it is real; `# ritsu: test secret` on the line of a value for tests |

[codes.md](codes.md) has every code with the smallest book that shows it, as `ritsu chobo explain
<code>` prints it.

## 6. For a machine

- `ritsu chobo check --format json <file.book>…` prints `{"v": 2, "files": [...]}`: each file's
  `diagnostics` (`code`, `severity`, `file`, `line`, `col`, `message`, `notes`, `excerpt`,
  `operations`, `hint`, `fix`) and its `report` (each operation's `refusals`, each with an
  `example` that gets there).
- `ritsu chobo api <file.book>` prints how to call the book, the reasons of each operation, the life of
  a hold as a state machine, how IDs are made, and what each target names things.
- The exit code is 0 when there is no error (warnings may be), 1 when there is, and 2 for a
  mistake in how chobo is called or a file it cannot read. A flag chobo does not know is exit 2.
- `--lang ja`, or `CHOBO_LANG=ja`, gives the messages in Japanese; the JSON keys stay English.

[formats.md](formats.md) has every JSON chobo reads and writes.

## 7. Targets

| Target | What `build` writes | What to know |
|---|---|---|
| `postgres` | a schema, tables, constraints and a function for each operation, in SQL | runs any number of times; a function runs in the caller's transaction; call `expire()` from a job |
| `postgres-typescript`, `postgres-python`, `postgres-go` | a client of those functions | pass it a `pg` client or pool, a DB-API connection, or a pgx conn or pool; it neither commits nor rolls back |
| `tigerbeetle-typescript`, `tigerbeetle-python`, `tigerbeetle-go` | a client that sends one chain of transfers for each operation | pass it the official client 0.17.9; a chain fits 253 transfers (E060) |

The clients of both databases have the same names, arguments and answers. A name of the book
that is Japanese is exported in Go with an `X` in front. [targets.md](targets.md) has what each
target writes, how to call it from the three languages, and where TigerBeetle answers otherwise
than the reference interpreter.

## 8. The files bundled with this skill

| File | What is in it |
|---|---|
| [reference.md](reference.md) | the whole language: units, accounts and bounds, transfers, operations, keys, holds, versions |
| [formats.md](formats.md) | the JSON of scenarios, results, `check --format json`, `api` and `run --show` |
| [targets.md](targets.md) | what `ritsu chobo build` writes for PostgreSQL and TigerBeetle, and how to call it |
| [codes.md](codes.md) | every diagnostic code, as `ritsu chobo explain --all` prints it |

They are copies of the pages under `crates/chobo/docs/` in <https://github.com/i2y/ritsu>, where
the examples and their pages are too.
