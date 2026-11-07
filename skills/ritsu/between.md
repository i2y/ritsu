# Between the languages

Each language answers for its own files, over every input, every day and every path. `ritsu check`
adds the relations a file says out loud: a `use`, a `from`, an `import enum`, a `fed by`. What is
left is the relations a project depends on and no file says. Those are the ones that break with no
diagnostic: a limit lowered on one side, a value renamed on one side, a hold that lasted only
because nothing took longer. `all pass` says nothing about them.

This page is how to find them and give each one a place where the check stops when it stops being
true. Do it when you are asked to look over a project, before you hand over one with hand-written
code between the languages, and when `ritsu check` passes on something you do not yet trust.

## 1. Ask the languages; do not read them

What a language answers is what its generated code does. What you conclude from reading its file is
a guess, so ask. Each language prints what a file declares, as JSON, and answers for one input.

| Language | What the file declares | What it answers |
|---|---|---|
| rulec | `rulec api <file>` | `rulec run <file> <name>=<value>…`, `rulec vectors <file>` for every row |
| dandori | `dandori api <file>` | `dandori run <file> --scenario <file.json>` |
| chobo | `chobo api <file>` | `chobo run <file> --scenario <file.json>` |
| koyomi | `koyomi api <file>` | `koyomi eval <file> <name>=<value>…`, `koyomi vectors <file>` |
| sekisho | `sekisho api <file>` | `sekisho vectors <file>` for every combination |
| kaname | `kaname api <file>` | `kaname run <file> <function> --input <file.json>` |
| sakai | `sakai api <file>` | `sakai vectors <file>`, what each layer's map says of each value |

```console
$ cd crates/sakai/examples/shop
$ rulec run billing/rules/billing_need.rule status=paid
handling = bill
$ koyomi eval delivery/ship_date.cal ordered=2026-07-10
ordered  2026-07-10 Fri
…
```

## 2. Find what nobody states

Look in three places. Write down each relation you find as its two sides, with `file:line`, and what
goes wrong when one side moves, with a concrete input. If you cannot name an input that goes wrong,
it is not a relation to hold; leave it out.

**The code around the generated code.** Wherever hand-written code takes a value from one language's
answer, or from a contract, and gives it to another, the code carries a relation no file says. Go
through the calls of generated code and the handlers. For each argument ask where it comes from, what
that side can give (`api`, `vectors`), and what this side takes (its `range`, its enum). Ask too what
the code does with each answer: an error it maps, a status it reads, a value it has a `default` for.

**Words written twice.** Search the whole project for the same thing in two files, by its value as
well as by its name:

- a list of values: the same names in two enums, an enum and a string in a `where`, a `.flow` and a
  `.gate`, a `.proto` and a table;
- an id: one thing keyed or named two ways (`key`, an id in a contract, a column of a table);
- a duration: `expires after`, `timeout`, `+ 30 days`, a wait, above all one that has to outlast
  another;
- a limit: a `range`, `at most`, a `maximum` in a schema, the same number in two languages, a bound on
  one side and none on the other;
- a unit: one amount in cents here and dollars there, tax in here and out there.

**Sentences.** A `description`, a comment, the `means` of a term, a README: a sentence that states a
relation ("held until it ships", "never more than the invoice"). Ask what would have to change for
the sentence to be false, and whether anything would say so.

## 3. Put each where a language can hold it

Most relations have a place in a language already. Writing it there is worth more than recording it
somewhere else, because that language then stops at the line that breaks it.

| What you found | Where it goes | What stops it |
|---|---|---|
| The same list of values in two or more of a rule, a flow and a gate | one `enums` file, and `import enum <enum> from "<file>"` in each | the reader's own check, at the line that names a value that has gone |
| A list a contract owns | `import proto` or `import jsonschema` in the rule or the `enums` file | rulec E032, E033 |
| A rule's input read from a contract's field | `from` on the input | rulec E121, E122 |
| A value hand-written code carries between two languages | `fed by <where it comes from>` at the end of the line that takes it | ritsu E207, W207, E208 |
| What a language says about its own answers: a date's number of days, a rule's result at a given input | koyomi `claims` and `examples`; a rule's `constraint`, `never`, `examples` | the language's own check |
| A combination of inputs that cannot happen | `never` in the rule | rulec E142 when the definitions give it, and the generated code at its door |
| How a layer between contexts reads an enum | the map's `enum <upstream> -> <downstream>` lines, and a test of the layer's code that reads `sakai vectors` | sakai E401 for the map, your test for the code |
| A hold a flow has to post or void in time | call the book with `use book` from the flow | ritsu E206, W206 |
| What a running service does | a claim in a `.geas` | geas, by running it |
| A requirement someone has to read again when a file changes | a link in a `.req` (`satisfied by`, `verified by`) with its `reviewed` record | yuen E302, E303 when an end changes |

Here is the first row, in `crates/sekisho/examples/order_states`. The enum is in one file, and the
flow and the gate read it. Rename a value in the file and both stop where they name it:

```text
error[dandori E003]: shipping.flow:17:5: `paid` is not a value of `order_state` (settled, shipped, returned, refunded)
error[sekisho E101]: orders.gate:26:26: The enum `order_state` has no value `paid`
```

Most of what you find is the fourth row, a value hand-written code carries. `fed by` names where the
value comes from, in the way the toolchain names anything, on the line that takes it. It goes on an
input of a rule, a workflow or a dates file, a field of `elements` or of a `record`, an attribute or
an input of a gate, and a field or a parameter of kaname, after a `from` if the line has one:

```rule
inputs
  food : number  range >=0 <=60  fed by rulec "province_income.rule" output food

elements units from battle.units
  veterancy : number  range >=0 <=2  from veterancy  fed by rulec "recruit_allowed.rule" output veterancy
```

A source can be a rule's `output`, a book's `account`, a `date`, a `.proto`'s `message … field`, an
OpenAPI `schema … property`, a JSON Schema's `def` or `pointer`, a workflow's `output` or
`record … field`, and a `record … field` of kaname. `ritsu check` asks the source what it can come to
and holds that to what the line takes. Here a book with no upper limit gives a record a value the
record refuses:

```text
error[ritsu E207]: refund.flow:9:1: `record Balance field left` takes money[USDc] >=0 <=10000000, and its source chobo "refunds.book" account remaining can be 10000001
     9 |   left : money[USDc]  range >=0USDc <=10000000USDc  fed by chobo "refunds.book" account remaining
  = The book takes it: an end with no `at least` or `at most` goes as far as chobo's limit (2⁶³ − 1).
```

Read the language's skill before you write a form you have not used ([which skill](SKILL.md#5-which-skill-to-read)).

## 4. What each line does not promise

Say what you have held and do not claim more:

- **`fed by`** checks that everything the source can give, the line takes. It does not check that the
  code carries that value: a `.geas` claim or a test does that. W207 means it could not be decided
  (rulec counts an end no input of the rule reaches, say). Keep the line, and list it as left open:
  it stops at the first value that does go past.
- **`import enum`** makes two files use one list of names. It does not make the names mean the same.
- **`sakai vectors`** gives the values. Nothing is held until a test of yours calls the layer with them.
- **`claims` and `examples`** hold for what they say. An example holds for its input.
- **A `reviewed` record** says a person read both ends once. E302 and E303 say an end has changed and
  someone must read it again. They do not say what is wrong, and they fire on any change. The record
  is the person's to write: do not run `ritsu yuen review` on your own judgment (the skill of yuen says
  why).

## 5. Break each one once

A line that has never failed is not known to hold anything. For each relation you wrote, make a copy
of the project, change one side the way someone would, and run `ritsu check`. It should stop at the
line you expect. If it does not, the line is not holding what you think: say so in the report rather
than count it. Delete the copy afterwards.

Do not change a range, a bound or a duration to make a relation pass. A relation that fails has found
something: report it with the input (§6). What the right number is belongs to the people who own it
([what stays with people](SKILL.md#7-what-stays-with-people)).

## 6. Report

Three lists, each with `file:line` on both sides.

1. **Held.** What you wrote, where, and what `ritsu check` said when you broke it.
2. **Failed.** A relation that does not hold today, with the input that shows it and what the project
   then does. Fix nothing here on your own judgment: this is what the person needs to read first.
3. **Left open.** A relation you found with no place to go, or one you could not decide: where it
   lives, what goes wrong if it moves, and that nothing checks it. An open list that says this is
   worth more than a clean report that does not.
