<!-- The output of `chobo explain --all --format markdown --lang en`. Do not edit by hand. -->

# chobo diagnostics

Every code chobo prints, when it appears, and how to fix it. For one of them: `chobo explain E020`.

| Code | Severity | Title |
|---|---|---|
| [E001](#e001) | error | A line that does not read |
| [E002](#e002) | error | A name that is not declared |
| [E003](#e003) | error | Declared twice |
| [E004](#e004) | error | The wrong number of arguments for an account |
| [E005](#e005) | error | A parameter used as the wrong type |
| [E010](#e010) | error | A move between units that differ |
| [E011](#e011) | error | A number that does not fit its unit |
| [E012](#e012) | error | A move from an account to the same account |
| [E013](#e013) | error | A transfer with no move |
| [E020](#e020) | error | An account with no bound |
| [E021](#e021) | error | An outside account with a bound |
| [E022](#e022) | error | Bounds that leave no balance |
| [E023](#e023) | error | A bound with no reason to refuse with |
| [E030](#e030) | error | A transfer with no key |
| [E031](#e031) | error | An amount in the key |
| [E040](#e040) | error | A hold that does not say how it ends |
| [E041](#e041) | error | An expiry out of range |
| [E050](#e050) | error | An account kind changed since the revision compared with |
| [E051](#e051) | error | A transfer kind changed since the revision compared with |
| [E060](#e060) | error | An operation that does not fit one TigerBeetle request |
| [E061](#e061) | error | A name too long for PostgreSQL |
| [W101](#w101) | warning | An account that only fills |
| [W102](#w102) | warning | A transfer that is always refused |
| [W103](#w103) | warning | A transfer refused for the order of its moves |
| [W104](#w104) | warning | A move of a hold that counts on another move of the same hold |
| [W105](#w105) | warning | Declared and never used |
| [W106](#w106) | warning | A bound that never matters |
| [W107](#w107) | warning | An account or transfer kind gone since the revision compared with |

## E001

`error` — **A line that does not read**

**When.** A word chobo does not know, an indentation that does not line up, a string left open, or a line that is not in one of the forms. A byte order mark, a tab and a combining mark (a voiced sound mark or an accent written as a character of its own) stop here too. With a line it cannot read, anything after would be a guess, so the names are not resolved and nothing else is checked.

**Fix.** Rewrite the line in the form the message gives. Under an account go `description`, `at least` and `at most`; under a transfer go `description`, `key`, `pending` and `move`. Write a letter with a combining mark as the one precomposed character.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
  keep 3
```

Related codes: [E002](#e002)

## E002

`error` — **A name that is not declared**

**When.** A unit, an account, a type or a parameter of the transfer is named that nothing declares. Names are compared exactly as written.

**Fix.** Declare it, or correct the name to one that is declared. An account for the world outside the book (a supplier, the customers) is declared too: `account supplier : pcs outside`.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
```

Related codes: [E001](#e001), [E004](#e004)

## E003

`error` — **Declared twice**

**When.** A unit, an account, a transfer or a parameter is declared twice under one name; or `description`, `at least`, `at most`, `key` or `pending` appears twice under one account or transfer; or the key lists a parameter twice.

**Fix.** Remove one, or give it another name. The key goes on one line.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
```

## E004

`error` — **The wrong number of arguments for an account**

**When.** A move gives an account more or fewer arguments than the account declares. Arguments are passed by position, in the order they are declared.

**Fix.** Give as many as the account declares: `stock(sku)` for `account stock(sku: string)`. An account without parameters is written with no parentheses at all: `customers`.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock
```

Related codes: [E002](#e002), [E005](#e005)

## E005

`error` — **A parameter used as the wrong type**

**When.** A string parameter is the amount of a move, an amount parameter is an account's argument, or an account declares a parameter of a type other than string.

**Fix.** An amount is a parameter whose type is a unit (`qty: pcs`), or a number; an account's argument is a string parameter, or a string.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move sku from supplier to stock(sku)
```

Related codes: [E010](#e010)

## E010

`error` — **A move between units that differ**

**When.** The account a move takes from, the account it puts into and its amount are not all in one unit. chobo does not convert.

**Fix.** Make them one unit. To exchange one unit for another, write two moves, each through an outside account of its own unit, with amounts the caller works out.

**Smallest reproduction**:

```book
book shop v1
unit pcs
unit yen
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : yen outside
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

Related codes: [E005](#e005)

## E011

`error` — **A number that does not fit its unit**

**When.** A number written in the book (a bound, or the amount of a move) has more decimal places than the unit's `scale`, an amount is negative, or a number is outside −(2⁶³ − 1) to 2⁶³ − 1, counted in the unit's smallest step.

**Fix.** Write it with no more decimal places than the unit's `scale`, and declare a unit that needs them as `unit USD scale 2`. To move the other way, swap `from` and `to`.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0.5 refused as out_of_stock
```

Related codes: [E022](#e022)

## E012

`error` — **A move from an account to the same account**

**When.** One move names the same account with the same arguments in `from` and in `to`. Every call is refused with same_account.

**Fix.** Move to another account; to move between two accounts of one kind, give them different arguments (`stock(from_sku) to stock(to_sku)`).

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer shuffle(note: string, sku: string, qty: pcs)
  key note
  move qty from stock(sku) to stock(sku)
```

## E013

`error` — **A transfer with no move**

**When.** A transfer has no `move` line.

**Fix.** Write where it moves from and to: `move <amount> from <account> to <account>`.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
transfer count(note: string)
  key note
```

## E020

`error` — **An account with no bound**

**When.** An account not marked `outside` has neither `at least` nor `at most`. The bounds of accounts are all chobo keeps, so such an account would keep nothing.

**Fix.** Give it a bound, like `at least 0 refused as out_of_stock`; or mark it `outside` if it stands for the world outside the book (a supplier, the customers, a bank).

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
```

Related codes: [E021](#e021)

## E021

`error` — **An outside account with a bound**

**When.** An account marked `outside` has `at least` or `at most`. An outside account stands for the world outside the book, and may go below 0.

**Fix.** Remove the bound; or, to keep that balance, remove `outside` so that the book keeps the account.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account supplier : pcs outside
  at least 0 refused as supplier_short
```

Related codes: [E020](#e020)

## E022

`error` — **Bounds that leave no balance**

**When.** The lower bound is above the upper bound, or the upper bound is below 0. An account starts at 0, so below 0 nothing could ever be put into it.

**Fix.** Make the lower bound no higher than the upper bound, and the upper bound 0 or more.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account shelf(sku: string) : pcs
  at least 10 refused as below_safety_stock
  at most 5 refused as shelf_full
```

Related codes: [E011](#e011)

## E023

`error` — **A bound with no reason to refuse with**

**When.** A bound has no `refused as <reason>`, or its reason is one of the names chobo gives itself (key_conflict, already_refused, same_account, no_such_hold, already_posted, already_voided, expired, over_hold). The reason is what a caller gets back when the bound refuses, and it is the name of the error of a dandori task.

**Fix.** Name it in the business's words: `at least 0 refused as out_of_stock`.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0
```

## E030

`error` — **A transfer with no key**

**When.** A transfer has no `key` line, or the key names something that is not one of its parameters. Every transfer needs an idempotency key: a second call with the same key and the same content does nothing, and one with other content is refused.

**Fix.** List the parameters it is to happen once for: once per order and SKU is `key order, sku`.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  move qty from supplier to stock(sku)
```

Related codes: [E031](#e031)

## E031

`error` — **An amount in the key**

**When.** The key lists a parameter whose type is a unit. A retry with another amount would then go through as a second transfer, and move twice.

**Fix.** Take the amount out of the key; a second call that differs only in the amount is then refused with key_conflict.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, qty
  move qty from supplier to stock(sku)
```

Related codes: [E030](#e030)

## E040

`error` — **A hold that does not say how it ends**

**When.** `pending` stands alone, with neither an expiry nor `never expires`. A hold ends when it is posted, voided or expires.

**Fix.** Write `pending expires after 30 minutes` or `pending never expires`. Only the caller ends a hold that never expires.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer reserve(order: string, sku: string, qty: pcs)
  key order, sku
  pending
  move qty from stock(sku) to customers
```

Related codes: [E041](#e041)

## E041

`error` — **An expiry out of range**

**When.** The expiry is under 1 second, over 4294967295 seconds (2³² − 1 seconds, about 136 years), or not a whole number. TigerBeetle's timeout is 32 bits of seconds.

**Fix.** Write a whole number from 1 second to 2³² − 1 seconds; to hold for longer than that, use `never expires`.

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer reserve(order: string, sku: string, qty: pcs)
  key order, sku
  pending expires after 0 minutes
  move qty from stock(sku) to customers
```

Related codes: [E040](#e040)

## E050

`error` — **An account kind changed since the revision compared with**

**When.** An account kind of the revision given to `--diff-base` has a new unit, `scale`, parameters or bound, or is outside where it was not (or the other way round). The book's version is not part of an account's identity, so the accounts that exist keep the old definition.

**Fix.** Declare an account under a new name, and write a transfer that moves the balances over. Make a limit that may change an account from the start (a member's credit limit becomes an account of credit left).

**The book at the revision compared with**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

**The book now (the smallest reproduction)**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 3 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

Related codes: [E051](#e051), [W107](#w107)

## E051

`error` — **A transfer kind changed since the revision compared with**

**When.** A transfer kind of the revision given to `--diff-base` has new parameters, a new key, a new way for its holds to end, or new moves. A retry in flight would be refused with key_conflict, and a hold still held could not be posted.

**Fix.** Declare the new form under a new name and keep the old one; remove the old one once every hold it made has ended.

**The book at the revision compared with**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

**The book now (the smallest reproduction)**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order
  move qty from stock(sku) to customers
```

Related codes: [E050](#e050), [W107](#w107)

## E060

`error` — **An operation that does not fit one TigerBeetle request**

**When.** With a TigerBeetle target of `chobo build`: one operation of a transfer kind sends a chain of more than 253 transfers. The chain has to go in one request to go through whole or not at all, and a replica started with `--development` takes at most 253 in one (8189 without it). A move between bounded accounts takes up to 6 transfers.

**Fix.** Split the transfer kind into kinds with fewer moves. Each kind is a write of its own, so keep together the moves that have to go through all or none.

**The command that shows it**: `chobo build <file.book> --target tigerbeetle-typescript`

**Smallest reproduction**:

```book
book split v1
unit yen
account pool(id: string) : yen
  at least 1 refused as pool_short
  at most 1000000 refused as pool_full
account credit(id: string) : yen
  at least -1000 refused as over_limit
  at most 1000 refused as overpaid
account bank : yen outside
transfer fund(note: string, id: string, amount: yen)
  key note
  move amount from bank to pool(id)
transfer repay(note: string, id: string, amount: yen)
  key note
  move amount from credit(id) to bank
transfer spread(note: string, a: string, b: string, amount: yen)
  key note
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
  move amount from pool(a) to credit(b)
```

Related codes: [E061](#e061)

## E061

`error` — **A name too long for PostgreSQL**

**When.** With a PostgreSQL target of `chobo build`: the name of the schema (the book's), of a function (`<transfer>_<operation>`, `balance_<account>`) or of a function's parameter (`p_<parameter>`) is longer than 63 bytes. PostgreSQL cuts a longer name short without a word, so two functions could end up with one name. A Japanese character is 3 bytes.

**Fix.** Make the name shorter: 63 ASCII letters, or 21 Japanese characters, less what follows it in a function's name (`_hold`, …).

**The command that shows it**: `chobo build <file.book> --target postgres`

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship_the_goods_to_the_customers_who_have_ordered_and_paid_for_them(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

Related codes: [E060](#e060)

## W101

`warning` — **An account that only fills**

**When.** Moves put into an account the book keeps, and no move anywhere takes out of it: whatever goes in stays.

**Fix.** Write a transfer that takes out of it, or mark it `outside` if it stands for the world outside the book.

**Smallest reproduction**:

```book
book shop v1
unit yen
account wallet(member: string) : yen
  at least 0 refused as short
account bank : yen outside
transfer top_up(slip: string, member: string, amount: yen)
  key slip
  move amount from bank to wallet(member)
```

Related codes: [W102](#w102), [W106](#w106)

## W102

`warning` — **A transfer that is always refused**

**When.** What a transfer takes from has a lower bound of 0 or more, and nothing anywhere puts into it: whenever the transfer moves more than 0, it is refused.

**Fix.** Write a transfer that puts into that account (a delivery, a top-up).

**Smallest reproduction**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account customers : pcs outside
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

Related codes: [W101](#w101)

## W103

`warning` — **A transfer refused for the order of its moves**

**When.** In a transfer that posts at once, an earlier move takes from an account that a later move puts into (a lower bound), or puts into one that a later move takes from (an upper bound). Moves are checked one at a time in the order they are written, so the earlier move is refused even when the two together would fit.

**Fix.** Write first the move the other one counts on: for a lower bound the one that puts in, for an upper bound the one that takes out.

**Smallest reproduction**:

```book
book market v1
unit yen
account buyers : yen outside
account fees : yen outside
account sales(shop: string) : yen
  at least 0 refused as sales_short
transfer sell(order: string, shop: string, price: yen, fee: yen)
  key order
  move fee from sales(shop) to fees
  move price from buyers to sales(shop)
```

Related codes: [W104](#w104)

## W104

`warning` — **A move of a hold that counts on another move of the same hold**

**When.** In a hold, one move takes from an account another move of the same hold puts into (or puts into one another move takes from). What is held cannot be spent where it comes in, nor makes room where it goes out, so changing the order does not help.

**Fix.** Have the amount in that account beforehand, or move it in another transfer after the hold is posted.

**Smallest reproduction**:

```book
book market v1
unit yen
account buyers : yen outside
account payouts : yen outside
account escrow(order: string) : yen
  at least 0 refused as escrow_short
transfer pay(order: string, price: yen)
  key order
  pending expires after 1 hour
  move price from buyers to escrow(order)
  move price from escrow(order) to payouts
```

Related codes: [W103](#w103)

## W105

`warning` — **Declared and never used**

**When.** A unit that no account or parameter uses, an account no move names, or a parameter that is in neither the key nor any move.

**Fix.** Remove it, or write where it is used.

**Smallest reproduction**:

```book
book shop v1
unit pcs
unit boxes
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

Related codes: [W106](#w106)

## W106

`warning` — **A bound that never matters**

**When.** Nothing puts into an account with an upper bound (or takes out of one with a lower bound), so the bound never refuses anything.

**Fix.** Remove the bound, or write the transfer it was meant to check.

**Smallest reproduction**:

```book
book shop v1
unit yen
account credit(member: string) : yen
  at least -1000 refused as over_limit
  at most 0 refused as overpaid
account shops : yen outside
transfer spend(slip: string, member: string, amount: yen)
  key slip
  move amount from credit(member) to shops
```

Related codes: [W101](#w101), [W105](#w105)

## W107

`warning` — **An account or transfer kind gone since the revision compared with**

**When.** An account or transfer kind of the revision given to `--diff-base` is gone, or the book has a new name. Its balances and holds stay in the database.

**Fix.** Move the balances out with a transfer before removing it, or keep it.

**The book at the revision compared with**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer ship(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

**The book now (the smallest reproduction)**:

```book
book shop v1
unit pcs
account stock(sku: string) : pcs
  at least 0 refused as out_of_stock
account supplier : pcs outside
account customers : pcs outside
transfer receive(note: string, sku: string, qty: pcs)
  key note, sku
  move qty from supplier to stock(sku)
transfer send(order: string, sku: string, qty: pcs)
  key order, sku
  move qty from stock(sku) to customers
```

Related codes: [E050](#e050), [E051](#e051)
