# Design

A `.book` is parsed, its names and units are resolved, and the typed book is checked: its units,
its bounds, its keys, its holds, and how things can move between its accounts. The reference
interpreter runs it, and the scenarios it plays are what every database is held to. Eleven
generators build it, and `chobo doc` and `chobo api` say what it is, to a person and to a tool.

## The bound of one account

The one condition a book can write is a lower or an upper bound of one account, and that is the
decision the rest stands on. A bound looks at one account, so the write that changes the account
can check it, with nothing else to read in between: on PostgreSQL, a function that locks the
account's row; on TigerBeetle, a flag of the account and a chain of transfers applied whole or not
at all; on the MySQL family, a transaction that locks the account's row as it reads it. A database
that can change a row on a condition and write several rows at once has what this needs; it is
tried on PostgreSQL, TigerBeetle, MySQL, TiDB and Vitess.

A condition over two accounts cannot be kept so. Two refunds at the same moment would each read the
sale and the refunds before the other wrote, and both would go through. So such a condition becomes the bound of
an account of its own:

| The condition | The account it becomes |
|---|---|
| a refund never exceeds the sale | what is left to refund on each order, `at least 0`: the sale puts in, a refund takes out |
| the stock of every SKU fits the warehouse | the room left in the warehouse, `at least 0`, which each delivery takes from in a second move |
| at most three refunds a month | the refunds left in each month, `at least 0`, in a unit of times |
| a member's credit line, which differs by member | an account of the credit left to use, which a transfer fills |
| the fee is 10% of the price | no bound: a rule works out the fee, and the transfer is given it |

What can bend is which quantities are accounts; the shape of the condition does not. It is the same
narrowing that lets rulec prove its tables complete: rulec keeps a cell to its own column, and chobo
keeps a condition to one account, so that every condition can be kept on every database it builds
for.

## Principles

- **P1. A bound of one account is the only condition.**
- **P2. A balance is what came in, less what went out.** There are no debits and credits to read:
  the book keeps quantities within bounds, and is not a general ledger. Every account counts the
  same way, so `at least 0` always means *never below 0*. The world the book trades with is
  accounts `outside` it, which go below 0 and have no bounds.
- **P3. An amount is a whole number of its unit's smallest step,** and a move is in one unit.
  chobo converts nothing.
- **P4. Every transfer is called by the key the book writes.** The same key with the same contents
  does nothing the second time; with other contents it is refused. The key is the book's, made of the
  transfer's own arguments, rather than an ID the caller makes and keeps: so the checker can say what
  a second call gets, the people who keep the accounts read *once per order and SKU* in the book,
  and two runs of one workflow cannot move twice.
- **P5. One reference interpreter says what a book means,** and PostgreSQL, TigerBeetle, MySQL,
  TiDB and Vitess are each held to it, scenario by scenario.
- **P6. chobo computes nothing.** An amount is a parameter or a literal; a transfer does not read a
  balance to decide one, and adds nothing up. That keeps every answer to *goes through, or is
  refused with a bound's reason*, so the reasons can be listed before anything runs.
- **P7. A hold ends posted, voided or expired.** The caller posts and voids it; expiry happens
  outside the caller's hands, on the clock.

## Decisions that follow

- **Moves are checked one at a time, in the order written.** TigerBeetle checks a chain transfer by
  transfer, and PostgreSQL and the MySQL family do the same, so they answer alike, and what a book
  means can be read from it. Checking the balances only after every move would mean reordering the
  chain behind the reader's back. The price is that the order can refuse what the moves together
  would allow, which the checker says (W103).
- **A hold is counted for the worst case.** What is held going out counts against a lower bound,
  what is held coming in against an upper one, and neither counts the other way, as TigerBeetle
  counts it. A hold that went through is never refused for a bound when it is posted.
- **A refusal is an answer, not a failure.** There is no stock: that is the business answering, and
  the clients return it as a result. A key a bound refused stays refused, as TigerBeetle forbids an
  ID it refused, so that a retry always gets the same answer.
- **chobo calls no other language.** A workflow reads the book through ritsu's port of books, and
  another tool through `chobo api`; chobo knows neither.
- **A client for each database, in each language.** The clients of every database have the same
  names, arguments and answers, so that changing the database changes how the book value is made and
  nothing else; and one runner in each language carries the tests through all of them. The Rust
  clients are async, as TigerBeetle's own client for Rust and sqlx are, and take what they call
  through a trait the drivers' pools, connections and transactions implement.
- **On the MySQL family, the client sends the statements.** TiDB has no stored procedures, and a
  service that runs Vitess may allow neither stored routines nor making a database, so there is no
  function to call: the client runs each call as one transaction of statements chobo wrote, in
  which the checks are, and fills in nothing but the values.
- **The build stops where its database would.** A chain longer than one TigerBeetle request of a
  replica chobo can be tested on (253 transfers), a name longer than PostgreSQL's 63 bytes, and a
  name MySQL cannot hold stop the build for that database, and no other; a name with a character
  no Rust identifier holds stops the builds for Rust.

## Dropped

- **Transfers that read a balance to decide an amount**: the answer would no longer be *goes
  through or is refused*. The caller reads the balance and decides, and the transfer is refused if
  the balance has changed under it.
- **Debits and credits**: see P2.
- **Keys the caller makes**: the checker could not say what a second call gets.
- **A bound that differs from account to account**: an account of the limit, filled by a transfer.
- **An interpreter in the database**, reading the book from a table: a function for each operation,
  which follows the book move by move, can be read beside the book.
- **A server of chobo's in front of the database**: the database keeps the write whole already; a
  server would be one more thing on the way.
- **A hold that posts itself when it expires**: TigerBeetle's expiry only lets a hold go. *Post it
  after 30 days* is a workflow's wait.

## The whole record

[DESIGN.md](https://github.com/i2y/ritsu/blob/main/crates/chobo/DESIGN.md), in Japanese, gives the
reasons for these and for every other decision, what TigerBeetle, PostgreSQL, MySQL, TiDB and
Vitess were seen to do before the design relied on it, the designs that were dropped, and what is left.
