# What the check guarantees

`kaname check` holds a module to the rules ritsu's languages are held to — units, currencies and
tax, the steps of rates, ranges, rounding and int64 — and works out, from the ranges declared, what
each value can be at each place it goes. `ritsu check` runs it on every `.kn` of a project, with the
rules, the dates files and the books the modules call. Every module of the examples passes with no
warning. This page shows what the check says, and how what it says is held to what the code does.

## Effects

Each function's type says how far it was checked:

| Effect | What it says |
|---|---|
| total | it answers every input in the ranges it declares |
| fails | it has places the check could not show, which its code checks when it runs; the kinds are said: `rule`, `date`, `range`, `overflow`, `zero` |
| diverges | it recurses, itself or through others; that it stops is not checked |

kaname infers the effect, and holds a function to the one its head writes (E401). A call carries
the effect of the function called to the caller, and a function given to another carries its own
to the function that gives it. The line that says a module passes sums the effects up:

```console
$ cd crates/kaname
$ ritsu kaname check examples/order_flow/order_flow.kn examples/catalog/catalog.kn tests/doc/decisions.kn
ok examples/order_flow/order_flow.kn: 5 functions, total
ok examples/catalog/catalog.kn: 9 functions, diverges
warning[W301]: tests/doc/decisions.kn:20:24: Whether `weight` stays in >=1oz <=240oz, the range of the input `weight` of the rule `media_mail`, cannot be decided
    20 |     media_mail(weight: weight, media_only: all(lines, l => l.media)).price
  = The analysis of ranges finds the value in >=0oz <=240oz, and no input among the function's vectors takes it out.
  = The generated code checks it there when it runs.
  = Narrow it with an `if`, which the analysis reads, and it is shown.
warning[W501]: tests/doc/decisions.kn:25:3: This `if` chooses a constant of the business by the values it compares (`0USD`, `5USD`): a decision, which belongs in a table
    25 |   if total >= 35USD then 0USD else 5USD
  = Write the decision in a rulec table and call the rule: there, rulec checks it is complete and without overlap, and the person who approves it reads it on its page.
ok tests/doc/decisions.kn: 3 functions: 2 total; 1 fails rule
```

The third module is a material of kaname's tests, not an example to follow. Its `quote` gives a
rule of Media Mail's prices a weight the check can neither keep inside the rule's range nor find an
input to take out of it: the check warns (W301), the generated code checks the weight there when it
runs, and the function is `fails rule`. Its `shipping` chooses a fee by comparing the total in an
`if`, a decision of the business, which belongs in a table of rulec, where its completeness and
overlap are checked and the person who approves it reads it on its page (W501). A function that only
narrows a value to a rule's range before it calls the rule is not warned of.

The effect shapes the generated code. In Go only a function that can fail returns `(T, error)`, and
in TypeScript and Python each function has the union of the kinds it can fail with in its type.

## A value whose range is not written

A value whose range is not written takes the whole of its type, and what is computed from it is
held to int64 as any other value is:

```kn
fn next_id(last: number) -> number total
  last + 1
```

```console
$ cd crates/kaname
$ ritsu kaname check tests/docs/ids.kn
error[E401]: tests/docs/ids.kn:4:1: `next_id` is written `total`, but it can fail with `overflow`
     4 | fn next_id(last: number) -> number total
  = Line 5: the value of `last + 1` can leave int64, as it comes from a value whose range is not written (`overflow`)
  = Make the places shown (narrow the values with an `if` before they go there), or write a wider effect.
```

`last` may be the largest number of int64, and one more is past it. An overflow that comes from the
end of a type is no error of its own: without `total` on its head, `next_id` passes as
`fails overflow`, and its code checks `last + 1` where it computes it. With a range written,
`last: number range >=0 <=1_000_000`, the place is shown and the function is total.

## Every value given, against where it goes

Each value given to a parameter, the answer of a function, a field of a record, an input of a rule
or of a date, and the amount of a transfer is held to the range of where it goes. Here the days an
invoice was paid late are given to a rule without the branch that narrows them to its range (the
example `late_payment`, on [Rules, dates and books](together.md), has the branch):

```kn
fn interest(invoice: Invoice, paid_on: date range >=2026-01-01 <=2029-12-31, reference_rate: rate[step 0.01%] range >=0% <=15%) -> money[GBPc]
  let due = terms.due(invoiced: invoice.invoiced)
  let late = paid_on - due
  charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate).interest
```

```console
$ cd crates/kaname
$ ritsu kaname check tests/docs/unnarrowed.kn
error[E301]: tests/docs/unnarrowed.kn:15:44: `late` can be outside >=1 <=1500, the range of the input `days_late` of the rule `charges`
    15 |   charges(debt: invoice.amount, days_late: late, reference_rate: reference_rate).interest
  = For example, `interest(invoice: Invoice { id: "a", invoiced: 2026-01-01, amount: 50000000GBPc }, paid_on: 2026-01-01, reference_rate: 0%)` gives it -29.
  = Narrow it to that range with an `if` before it goes there, or widen the range it goes to.
```

An error comes with an input of the function, found among its vectors or solved for, that puts the
value outside: the invoice paid on the day it was invoiced, 29 days before it fell due. The end of a
range the analysis estimates is never given as one, since it may not happen. A place the check can
neither show nor break with an input is a warning, as W301 above, and the code checks it there when
it runs.

## The code generated answers as the interpreter does

The meaning of a module is its reference interpreter's, which `kaname run` runs and `vectors`
chooses inputs for. The code `ritsu gen` writes is held to it in TypeScript, Python and Go: on every
vector of 52 modules of the examples and the tests (13,982), on every input just outside what their
parameters declare, and on values of the types that hold themselves nested 200 deep, as deep as a
host may give one, and 201. In English and in Japanese, all 16,717 cases agree in TypeScript, 16,633
in Python and 16,613 in Go (an input past the whole of a type is run where the language can hold
it), and the code passes `tsc --strict --erasableSyntaxOnly`, `mypy --strict`, gofmt and `go vet`.
The hosts of the four [functional cores](examples.md) print what their `output.txt` holds.

## The analysis of ranges, against what runs

The analysis says what each expression can be. Every function of the examples and the test modules
is run on its vectors and on 200 inputs drawn from what its parameters declare: on 139,754 runs, the
3,452,025 values that 5,009 expressions came to are each inside what the analysis says, and each of
the 1,617 runs that fail fails in a way the function's effect says.

## Lean

kaname's evaluation and its analysis of ranges are written again in Lean (`proofs/KanameCore`, in
ritsu's one Lean package), with proofs that the analysis is sound and that a function it infers
`total` fails on no input in the ranges it declares, for every function that takes no function as a
parameter; a function that does is held to the model by the comparison below, with no proof of its own
yet. No proof rests on `sorry` or an axiom of its
own. `cargo test -p ritsu-model` holds the model to the reference interpreter and the analysis on
14,092 runs of 59 modules, place by place and expression by expression, with no difference; and
the model broken in any one of 42 places, the comparison fails each time.
