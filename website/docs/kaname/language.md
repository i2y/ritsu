# Writing kaname

A module (a `.kn` file) starts with its name and what it is for, then the files it reads, then its
records, enums and functions in any order, and last the `examples` its functions must answer. A
function is `let` lines and then its answer: there is no statement, no variable that changes and no
loop, and a value is never changed once it is made. This page goes through what the four examples
of a functional core are written with. [The reference](https://github.com/i2y/ritsu/blob/main/crates/kaname/docs/reference.md)
has the whole language.

## Ranges, written or not

The types are ritsu's: a number, money and quantities in their units, a rate with its step, a date,
a truth value and a string. A field or a parameter may write the range of its values, and a list or
a string the count of its elements; it may also leave them out. The lines of an order write them:

```kn
record Line
  sku   : string       count >=1 <=20
  qty   : number       range >=1 <=100
  price : money[USDc]  range >=1USDc <=1_000_000USDc
```

A range written is what the host promises: the code a host calls checks each argument against it
before it computes, and fails with `input` outside it. A value whose range is not written takes the
whole of its type: all of int64, in its unit, for a number, an amount or a quantity, and every day
from 0001-01-01 to 9999-12-31 for a date. The total of a placed order, its date and the tracking
number of a shipped one have no range written. What is computed from such a value is held to int64 like
anything else, and where the check cannot show it stays inside, the code checks it when it runs
([What the check guarantees](checks.md)).

## Enums with fields, and `match`

An enum lists its values under it, and a value may have fields of its own. The events of an order
are such an enum:

```kn
enum Event
  add
    line : Line
  remove
    sku : string
  place
    on : date
  pay
    amount : money[USDc]
  ship
    tracking : string
  cancel
    reason : string
```

A value is made with its fields, `Order.placed { lines: c.lines, total: …, on: p.on }`, and taken
apart by `match`, an arm a line, which names the value it takes to read its fields (`placed p`, then
`p.total`). An arm may take more than one value, and `else` takes every value the arms above leave:

```kn
# whether an order takes more events
fn open(order: Order) -> bool
  match order
    shipped, cancelled => false
    else => true
```

A value no arm takes, an arm for a value an arm above took, and an `else` with nothing left are
E106. A number, an amount, a date or a string is split by literals, with `else`. `=` and `!=` compare
two values of one type whole: a record field by field, a list element by element, a value of an
enum by its value and its fields. `{ ..c, children: kept }` is a record made from another, with the
fields written changed.

## Dicts and sets

`dict[K, V]` holds a value for each key, and `set[T]` each member once. A key is a number, an amount,
a quantity, a rate with a step, a date, a string, a truth value or a value of an enum without
fields, and a dict and a set go round in the order of their keys, whatever order they were made in.
A builtin is written after the value it takes first:

```kn
# every code that differs, in the order of the codes: a dict and a set go round in their keys' order
fn changes(before: dict[string, number] range >=0 <=1_000_000, after: dict[string, number] range >=0 <=1_000_000) -> list[Change]
  let skus = before.keys().to_set().union(after.keys().to_set())
  skus.to_list().filter_map(sku => change(sku: sku, before: before.get(sku), after: after.get(sku)))

# the moves of a day, netted for each code
fn net(moves: list[Move] count <=1_000) -> dict[string, number]
  moves.group_by(m => m.sku).map_values(ms => ms.sum(m => m.qty))
```

The range of a dict is the range of its values, so every count here is 0 to 1,000,000 units.
`ritsu kaname run` runs a function in the reference interpreter on an input of JSON. The input
gives the counts as pairs, in another order than the codes' (`[["ink", 4], ["pen", 10], ["pad", 2]]`),
and `--trace` shows each `let`:

```console
$ cd crates/kaname
$ ritsu kaname run examples/stock_diff/stock_diff.kn changes --input tests/inputs/stock_diff.json --trace
changes(before: ["ink": 4, "pad": 2, "pen": 10], after: ["pad": 2, "pen": 7, "tape": 3])
  line 34: let skus = ["ink", "pad", "pen", "tape"].to_set()
= [Change.removed { sku: "ink", qty: 4 }, Change.changed { sku: "pen", was: 10, now: 7 }, Change.added { sku: "tape", qty: 3 }]
```

The code generated walks a `Map` of TypeScript, a `dict` of Python and a `map` of Go in the same
order, so the answer is the same list in every target.

## Strings and `result`

A string is a sequence of code points. Its length, a cut of it and the order of two strings are in
code points, in every target. White space is Unicode's, and the case a builtin changes is that of
the letters of ASCII. No builtin of a string fails: `parse_number` answers `none` for a string that
is not a number.

`result[T, E]` answers a value (`ok`) or what the business refuses it with (`err`).
`let x = try e` takes the value of an `ok`, and answers an `err` of `e` as the function's own:

```kn
# the handle as it is kept: trimmed, each run of white space inside one hyphen, A to Z as a to z
fn normalized(raw: string) -> string
  raw.words().join("-").lower_ascii()
…
# a tag written `<handle>#<number>`: the handle made as `handle` makes it, the number from 1 to 9999
fn tag(text: string) -> result[Tag, list[string]]
  match text.index_of("#")
    none => err(["no # between the handle and the number"])
    some i =>
      let h = try handle(raw: text.take(i))
      match text.drop(i + 1).parse_number()
        none => err(["the number is not written in digits"])
        some n => if n >= 1 and n <= 9999 then ok(Tag { handle: h, number: n }) else err(["the number is not from 1 to 9999"])
```

An `err` is an answer, not a failure: `tag` is total. What kaname calls a failure is a place the
check could not show, which the function's effect says and nothing in the language catches.

## Generics and function values

A function, a record and an enum may take type parameters (`collect[T]`; `K: key` holds one to the
types that can be keys). A parameter may be a function, given a lambda or a function by its name:

```kn
# the tree with each category's name made by the function given
fn relabel(c: Category, label: (string) -> string) -> Category
  Category { ..c, name: label(c.name), children: c.children.map(x => relabel(c: x, label: label)) }

# the names as the parts of a web address take them: words joined by hyphens, in lower case
fn slugged(c: Category) -> Category
  relabel(c: c, label: n => n.words().join("-").lower_ascii())

# what the function given picks from each category, from the top down
fn collect[T](c: Category, pick: (Category) -> list[T]) -> list[T]
  pick(c).concat(c.children.flat_map(x => collect(c: x, pick: pick)))

fn items(c: Category) -> list[Item]
  collect(c: c, pick: x => x.items)

fn names(c: Category) -> list[string]
  collect(c: c, pick: x => [x.name])
```

A function with type parameters or with a parameter of a function type is called by the module's
functions and by no host: a host calls `slugged`, `items` and `names`, whose types are all known.
Where such a function is called, the check reads its body again with the values and the functions
the call gives. A function given carries its effect to the function that gives it, and a function
type may write the effect it allows (`(string) -> string total`); a function given that does more is
E402. No function answers a function, and no record, list or dict holds one.

## Types that hold themselves

A record or an enum may hold itself, through a list, a dict, a set, an optional, a `result` or
another type. A category of a catalog holds the categories under it:

```kn
record Category
  name     : string
  items    : list[Item]
  children : list[Category]
…
# the tree with every category that holds no item, under it or in it, taken out
fn pruned(c: Category) -> Category?
  let kept = c.children.filter_map(x => pruned(c: x))
  if c.items.count() = 0 and kept.count() = 0 then none else Category { ..c, children: kept }
```

A type each value of which would hold another of it, such as a record with a field of its own type
that is neither optional nor in a list, is E108. A function that walks such a value recurses, so
its effect is `diverges`: kaname does not check that it stops. A host may give a value nested 200
deep, counted one for each value of such a type along a path, and a deeper one fails with `input`
before the function starts.

## What is not in it

Input and output, the clock, random numbers and state that changes are the host's: it reads them,
and gives the values to the functions it calls. There is no floating point (a value travels as a
whole number inside int64, and is an exact fraction until it is rounded), no loop (`reduce` and
recursion instead), no function that answers a function, no regular expression, and no way to catch
a failure (a refusal of the business is a `result`).
[DESIGN 29](https://github.com/i2y/ritsu/blob/main/crates/kaname/DESIGN.md) gives the reason for
each, in Japanese.
