---
title: "Write rules. Prove them. Compile them."
hide:
  - navigation
  - toc
---

<div class="rc-hero" markdown>
<img class="rc-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="rc-hero__mark" src="images/mark-light.svg#only-light" alt="">

# rule<span class="rc-hero__c">c</span>

<p class="rc-hero__tag">Write rules. Prove them. Compile them.</p>

<p class="rc-hero__lede">
<strong>A little language for business rules</strong> — a shipping tariff, a coupon
policy, an eligibility test, a tax table with its reduced rates and provisos. Conditions
are written as tables, and around them go calculations, exceptions that take precedence
over a main rule, provisos, a rule applied to another case, and lists whose length is not
fixed.
</p>

<p class="rc-hero__lede">
<strong>Little on purpose.</strong> There is no recursion and no state, and a cell looks at
its own column and nothing else. That is what lets rulec prove that every input in the
declared domain gets exactly one answer — shown over all of them, not sampled by tests. Only
a rule that passes compiles, into ordinary functions in twelve languages with no runtime and
no dependencies.
</p>

<p class="rc-hero__lede">
<strong>An agent can do the writing.</strong> What it hands back is a rule a person can read,
not code. What fails the checks comes back with the input that causes it, and what only a
person can decide comes back as a question.
</p>

<div class="rc-hero__cta" markdown>
[Try it in the browser](playground.md){ .md-button .md-button--primary }
[Install](install.md){ .md-button }
[Write a rule (.rule)](tour.md){ .md-button }
[For agents](agents.md){ .md-button }
[GitHub](https://github.com/i2y/rulec){ .md-button }
</div>
</div>

<div class="rc-overview" markdown>
![Write the table. rulec turns each row into a box in the input space and proves by computation that the boxes leave no gap and no overlap. If there is a gap, back comes the input that falls through it (Destination = Overseas, Weight = 2001g): add the row and run again — only what the fee is takes a person. Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy](images/overview.svg?v=a079778b#only-dark)
![Write the table. rulec turns each row into a box in the input space and proves by computation that the boxes leave no gap and no overlap. If there is a gap, back comes the input that falls through it (Destination = Overseas, Weight = 2001g): add the row and run again — only what the fee is takes a person. Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy](images/overview-light.svg?v=a079778b#only-light)
</div>


---

## What rulec does

<div class="rc-row" markdown>
<div markdown>

```rule
table FeeTable(fee_table)
policy unique
| Destination | Weight     | -> Fee(fee) : money[USD, incl_tax] |
| Domestic    | <=2kg      | 8USD                               |
| Domestic    | >2kg <=5kg | 10USD                              |
| Domestic    | >5kg       | 13USD                              |
| Overseas    | <=2kg      | 12USD                              |
| Overseas    | >5kg       | 20USD                              |
```

</div>
<div markdown>

### Built on tables, so it can be proved

Conditions are written in tables, and a cell tests the value in its own column and nothing else. So each row is one box in the space of inputs, and whether the boxes leave a gap or overlap can be computed exactly. The same table is the specification a person reads and approves, and in the generated code each row is one branch. This one is the table in the picture above, and **it is a row short**.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

```console
error[E101]: Completeness gap: some input matches no row
  --> fee.rule:21 table FeeTable
   |
21 | table FeeTable(fee_table)
   |       ^^^^^^^^ the input space is not fully covered
   |
 An input that matches no row: Destination = Overseas, Weight = 2001g
```

</div>
<div markdown>

### Gaps and overlaps fail before anything runs

Not sampled: the whole declared range is walked. The missing row comes back as **the input that falls through it**, so the fix is one row, and what the fee *is*, only a person can say.

</div>
</div>

<div class="rc-row" markdown>
<div markdown>

```console
error[E104]: An unrounded value reaches the output
  --> parcel.rule:19
   |
19 |   fee : money[USD, incl_tax]
   |         ^^^^^^^^^^^^^^^^^^^^ no rounding is declared
   |
 Example: some input computes to 35.5USD. down(1USD) gives 35USD,
 half_up(1USD) gives 36USD and up(10USD) gives 40USD, so the rounding
 mode moves the result by up to 5USD.

error[E103]: This column is length[in], but `22lb` is written here
  --> parcel.rule:26 table size_of
   |
26 | | <=22lb | envelope             |
   |   ^^^^^^
   |
 `22lb` is not a unit of length[in].
```

</div>
<div markdown>

### Units and tax flags are types. Rounding has to be declared

A value carries its unit, and an amount its currency and whether tax is included, so dollars and grams will not add and a tax-inclusive amount will not pass for a tax-exclusive one. Every intermediate is shown to fit in int64. A numeric output has to say how its fractions settle, and the question comes **with the money the choice moves**: nothing is rounded silently.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

```rule
table size_of
policy first
| girth  | -> size : size_class |
| <=22in | envelope             |
| <=60in | small                |
| -      | large                |

table base_rate
policy unique
| dest          | size     | weight  | -> base : money[USD, incl_tax] |
| north_america | envelope | -       | 6USD                           |
| north_america | small    | <=160oz | 12USD                          |
| north_america | small    | >160oz  | 18USD                          |
…
```

</div>
<div markdown>

### Tables stack

Because a cell sees only its own column, tables stack as deep as you like. Here one table decides the size class from the girth, and the next reads that class as a column of its own to price the parcel. Every table in the stack is checked the same way, a row that names a value the table above never produces is reported as one nothing reaches, and the answer comes back with the row that decided it in each table.

</div>
</div>

<div class="rc-row" markdown>
<div markdown>

```rule
table by_age
policy unique
| age       | -> hourly : money[GBPc] |
| <18       | 800GBPc                 |
| >=18 <=20 | 1085GBPc                |
| >=21      | 1271GBPc                |

table apprentice_rate
policy unique
overrides by_age
| apprentice | first_year | age  | -> hourly : money[GBPc] |
| true       | -          | <19  | 800GBPc                 |
| true       | true       | >=19 | 800GBPc                 |
```

</div>
<div markdown>

### Exceptions and provisos are checked with the main rule

A rule is rarely one table. GOV.UK gives a minimum wage for each age band, then states the apprentice rate as an exception to it, so the exception is a table of its own that `overrides` the first. A proviso that reads as a sentence is a `clause`, and a calculation gets a name with `define`. The checks read the main rule and its exceptions as one: every input still has to land somewhere, and where two could apply, `overrides` has to say which wins, or the check fails.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

```console
$ rulec certificate fee.rule > cert.json
$ proofs/.lake/build/bin/rulec-recheck --rule fee.rule cert.json
Fee (0.19.1), re-checked against the Lean proofs
  FeeTable: 6 rows — complete, 6 rows reached, no two rows meet, 1 axes tiled
    12 boxes read back from the cells they were written as
  the digest is fee.rule's, and 12 cells are read back out of it
OK: every claim this program states was proved, by the theorems of RulecCert.

$ rulec test generated/ --proofs
…
ok    fee (Rust, proof) 2 harnesses
```

</div>
<div markdown>

### The proofs are checked again, outside rulec

`rulec certificate` prints what the proofs rest on, and two programs that share no code with rulec check it again: one dependency-free Python file, and a checker built from a [Lean 4](https://lean-lang.org/) development in which it is a theorem that passing its checks makes the claims true. The generated Rust goes to the model checker [Kani](https://model-checking.github.io/kani/), which shows over **every** input in the declared domain that no table falls through and nothing overflows.

</div>
</div>

<div class="rc-row" markdown>
<div markdown>

```rule
source gov = file "sources/uk-sdlt.md" sha256:799106ea8a821a00  # GOV.UK, Open Government Licence v3.0
  table1 sha256:360409a5675d3552
  table2 sha256:15d4ed0baca64189

table standard
policy unique
| price                   | -> band : rate[step 1%] |
| <=125000GBP             | 0%                      |
| >125000GBP <=250000GBP  | 2%                      |  @gov table1
| >250000GBP <=925000GBP  | 5%                      |  @gov table1
| >925000GBP <=1500000GBP | 10%                     |  @gov table1
| >1500000GBP             | 12%                     |  @gov table1
```

</div>
<div markdown>

### Cite the source, and pin the copy

Declare the document a rule was transcribed from with `source`, and cite the table each row came from — `@gov table1`, here GOV.UK's stamp duty rates. `rulec source fetch` keeps a copy beside the rule, and `rulec source pin` fixes it by its hash. From then on the rows are held to that copy: **one wrong digit fails**, and a figure the copy states that no row uses is W120, the other half of the same slip. A statute is cited by its section and pinned to the government's own text, on e-Gov or the eCFR, and an amendment names the rows to reread.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

![The approver's page for the shipping-fee rule. Example 2 (東京都, 1999g, 12000円, プラチナ) is in the form on the left and the answer reads 送料 = 400円, with the line the generated code would log under it; to the right, row 3 of 基本送料 (not a remote area, up to 2000g) and row 2 of 負担判定 (platinum) are lit](images/try-top-en-dark.png#only-dark)
![The approver's page for the shipping-fee rule. Example 2 (東京都, 1999g, 12000円, プラチナ) is in the form on the left and the answer reads 送料 = 400円, with the line the generated code would log under it; to the right, row 3 of 基本送料 (not a remote area, up to 2000g) and row 2 of 負担判定 (platinum) are lit](images/try-top-en.png#only-light)

</div>
<div markdown>

### There is a page for the person who approves

`rulec doc --format html` renders the rule as one page an approver tries a case on: type a case, and the rows that matched light up and the answer appears. It is the generated JavaScript itself that runs, so the page says nothing the code does not. The document to approve, with the cited copy set beside the rule's own table, and the article a help centre publishes come from the same rule.

</div>
</div>

<div class="rc-row" markdown>
<div markdown>

```python
if dest in _north_america and size == SizeClass.ENVELOPE:  # row 1
    base = 6
elif dest in _north_america and size == SizeClass.SMALL and weight <= 10:  # row 2
    base = 12
...
else:
    raise AssertionError("unreachable: completeness was statically checked")
```

</div>
<div markdown>

### Twelve languages, not a dependency between them

Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm, and NumPy for whole columns at once. **One row, one branch**, with no runtime and no configuration. Beside each function, a `_traced` twin returns **which row of which table decided the answer**, which is what a log line or a reply to a customer needs.

</div>
</div>

**Twelve targets** · **86 diagnostics** · **48 rules checked, generated and run on every commit — 21 transcribed from a published source** · **no dependencies, no runtime** · **one binary** · **the checks are offline**

---

## Where to start, by what you have

None of the three changes anything that runs today.

| you have | the first move | the command |
|---|---|---|
| **a spreadsheet, a published policy or a statute** | Transcribe it into a `.rule` and check it. A workbook gives a first draft straight out of the file, with every guess marked. A statute or a document is cited and kept as a copy. No data and no old implementation are needed | `rulec import xlsx`, then `rulec check` — [What it proves](checks.md) |
| **an implementation that runs today** | Transcribe it into a `.rule` and wrap the old code in a short adapter. `verify` runs the cases built from the rule's own boundaries through both, and returns where they disagree, clustered by the rows that matched. The running code is not touched | `rulec verify` — [Compare and replay](compare.md#against-a-legacy-implementation) |
| **past records** | Replay the rule over them. For a change, how many records move and by how much comes out before it ships. Which inputs move at all needs no records, only the two versions | `rulec fixtures lint`, then `rulec replay` / `rulec diff` — [Compare and replay](compare.md#against-what-actually-happened) |


## Who writes it — a person, an agent, and rulec

Today a rule sits in a spreadsheet, a published policy, a wiki page or somebody's head, and an
engineer rewrites it as a chain of `if`s. rulec hands that rewrite to an agent, and changes
what the agent hands back: **a rule a person can read, instead of code**.

![The agent writes the rule, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy functions generated from the proved rule, and the impact known before you deploy](images/flow.svg?v=a079778b#only-dark)

![The agent writes the rule, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy functions generated from the proved rule, and the impact known before you deploy](images/flow-light.svg?v=a079778b#only-light)

Indigo is the loop between the agent and rulec, and it runs without a person: the agent hands
over the rule, rulec hands back what is wrong — where, how to fix it, and an input that shows
it — and the agent fixes it and hands it over again. Amber is the detour through a person, who
is asked only what cannot be derived from the source — *"what is the fee for a small parcel
going overseas?"* — and answers with an amount or a rounding direction, never with code. What
they approve is a document rendered from the rule, with a page to try a case on.

The agent's procedure is [For agents](agents.md). The
[agent skill](https://github.com/i2y/rulec/tree/main/skills) ships in the repository
([how](install.md#the-agent-skill)), `rulec mcp` offers the same commands as tools where there
is no shell ([how](install.md#the-mcp-server)), and findings come back as JSON whose codes and
shape **stay put while the wording improves**.

---

## What gets proved, and what does not

<div class="rc-overview" markdown>
![One computation decides three defects: a gap (E101) is a stretch no row covers, an overlap (E105) is a stretch two rows both cover, and an unreachable row (E102) is one whose whole stretch the earlier rows take first. Same table in all three; one thing changed](images/checks.svg?v=a079778b#only-dark)
![One computation decides three defects: a gap (E101) is a stretch no row covers, an overlap (E105) is a stretch two rows both cover, and an unreachable row (E102) is one whose whole stretch the earlier rows take first. Same table in all three; one thing changed](images/checks-light.svg?v=a079778b#only-light)
</div>

Seven things are settled before anything is generated. **Five are proved statically** —
every input matches some row, no input matches two, no row matches nothing, units are never
confused, every intermediate fits in int64. **One is a declaration that has to be there** —
how fractions are settled, because which way is right is a business decision. **One is run** —
every worked example holds. If any of the seven cannot be shown, nothing is generated.

**What is *not* proved matters just as much**: that the rule matches reality, that the
generated code answers like the rule — that is tested, byte for byte in each generated
language, not proved — the row pairs the overlap proof could not reach, and that the checker itself is
right. Where each layer stops is on [What it proves](checks.md) and
[How it is checked](assurance.md).

## What you can write in it, and what you cannot

**A rule that decides one case, in one go, from flat facts**, and hands back an amount, a
yes/no, a class or an order: a tariff, a discount, a rate, an eligibility test, a
classification, a routing decision, a statutory provision. Money is not required.

The language has two parents. The conditions come from decision tables, by way of DMN; a main
rule with its exceptions and provisos comes from law written as logic, by way of Catala. What
it leaves out — recursion, state, date arithmetic,
nested objects, and any cell that looks at two columns at once — is what keeps every check
decidable. A cell narrows its own column and nothing else, so a row is a box, and whether boxes
leave a gap or overlap can be computed exactly. `weight × 10 > order total` cannot be a cell;
named with a `derive` or a `define`, it becomes a column of its own.

Whether yours fits — five questions, and how rulec differs from DMN, rules engines and
Catala — is on [Does your rule fit](fit.md).

## Where to read next

| page | what is on it |
|---|---|
| [Write a rule (.rule)](tour.md) | the language, from one table to exceptions, provisos and lists |
| [What it proves](checks.md) | the seven checks, and how to read a diagnostic |
| [Generate and call](generate.md) | the twelve targets, and the rule as an MCP tool or a service |
| [Compare and replay](compare.md) | against what runs today, and against past records |
| [How to use it, by role](scenarios.md) | a public rule, your own rule, a new one, and an API contract |
| [Examples](examples.md) | rules transcribed from tariffs, terms and statutes |
| [Does your rule fit](fit.md) | five questions, and what else is out there |
