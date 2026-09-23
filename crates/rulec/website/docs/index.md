---
title: "Write the table. Ship the proof."
hide:
  - navigation
  - toc
---

<div class="rc-hero" markdown>
<img class="rc-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="rc-hero__mark" src="images/mark-light.svg#only-light" alt="">

# rule<span class="rc-hero__c">c</span>

<p class="rc-hero__tag">Write the table. Ship the proof.</p>

<p class="rc-hero__lede">
<strong>A small language for table-shaped business rules, and a harness for the
agent that turns them into code</strong>.
</p>

<p class="rc-hero__lede">
A business rule — a shipping tariff, a coupon policy, an eligibility
test, a tax table — is written as one table a domain expert can read; rulec proves the
table has no gaps, no contradictions and no dead rows, and then
generates ordinary Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL and Wasm with no runtime to install, and a NumPy plan for a host that decides whole columns at once.
<strong>The proof happens before the code exists</strong>: a rule that
cannot be proved does not generate.
</p>

<p class="rc-hero__lede">
Proved here means <strong>shown over every input in the declared domain</strong> — not
sampled by tests, and not a mathematical argument written out by hand. Where the proof
fails, the input that breaks it comes back with the finding. How it is done is in
<a href="checks/">What it proves</a>.
</p>

<div class="rc-hero__cta" markdown>
[Try it in the browser](playground.md){ .md-button .md-button--primary }
[Install](install.md){ .md-button }
[Write a table (.rule)](tour.md){ .md-button }
[For agents](agents.md){ .md-button }
[GitHub](https://github.com/i2y/rulec){ .md-button }
</div>
</div>

<div class="rc-overview" markdown>
![Write the table. rulec turns each row into a box in the input space and proves by computation that the boxes leave no gap and no overlap. If there is a gap, back comes the input that falls through it (Destination = Overseas, Weight = 2001g): add the row and run again — only what the fee is takes a person. Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy](images/overview.svg?v=9abfb225#only-dark)
![Write the table. rulec turns each row into a box in the input space and proves by computation that the boxes leave no gap and no overlap. If there is a gap, back comes the input that falls through it (Destination = Overseas, Weight = 2001g): add the row and run again — only what the fee is takes a person. Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy](images/overview-light.svg?v=9abfb225#only-light)
</div>

That is the whole of it in one picture. A table goes in; rulec turns each row into a box,
and only a table whose boxes leave no gap and no overlap comes out as code. The picture
shows a gap, but **the same computation decides overlaps and rows nothing reaches** — the
three are set side by side further down.


---

## What rulec does for you

<div class="rc-row" markdown>
<div markdown>

```console
error[E101]: Completeness gap: some input matches no row
  --> parcel.rule:30 table base_rate
   |
30 | table base_rate
   |       ^^^^^^^^^ the input space is not fully covered
   |
 An input that matches no row: dest = overseas, size = small, weight = 1lb
 The shape of the row to add: `| overseas | small | 1lb | 6USD |`
```

</div>
<div markdown>

### Gaps and overlaps fail before anything runs

Not sampled: the whole declared range is walked. What fails comes back with **the input that causes it**, so the fix is one row. What the amount *is*, only a person can say.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

```console
error[E104]: An unrounded value reaches the output
   |
19 |   fee : money[USD, incl_tax]
   |         ^^^^^^^^^^^^^^^^^^^^ no rounding is declared
   |
 Example: some input computes to 35.5USD. down(1USD) gives 35USD,
 half_up(1USD) gives 36USD and up(10USD) gives 40USD, so the rounding
 mode moves the result by up to 5USD.
```

</div>
<div markdown>

### Dollars and grams will not add. Rounding has to be declared

The unit is part of the type, and tax-inclusive is not tax-exclusive. A numeric output must say how fractions settle, and the question comes **with the money the choice moves**. Nothing is settled silently.

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

### Twelve targets, not a dependency between them

Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm, and NumPy for whole columns at once. **One row, one branch** — no runtime, no configuration.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

```pycon
>>> uk_minimum_wage_traced(age=20, apprentice=True, first_year=False)
(1085, [Fired(table='by_age', row=2)])

>>> uk_minimum_wage_record(...)
{"in":{"age":20,"apprentice":true,"first_year":false},
 "observed":{"hourly":1085},"trace":[{"table":"by_age","row":2}]}
```

</div>
<div markdown>

### "Why this amount?" has an answer

Beside every function is a `_traced` twin that returns, with the answer, **which row of which table matched** — which is what a log line or a reply to a customer needs. `_record` writes the same thing as one line of fixtures.

</div>
</div>

<div class="rc-row" markdown>
<div markdown>

```console
$ rulec coverage rules/income_tax.rule
17 vectors
  row coverage                7 / 7     satisfied
  boundary-pair coverage     12 / 12    satisfied
  rounding-tie coverage       1 / 1     satisfied
```

</div>
<div markdown>

### The test cases build themselves, from the boundaries

Vectors come from the table's own edges, shadowed pairs and rounding ties, and run through the reference evaluator and all twelve languages **byte for byte**. Then a separate judge audits the suite itself.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

```console
error[E116]: The amount of row 3 is not in the copy it cites
   |
27 | | >=21      | 1272GBPc                |
   |               ^^^^^^^^ not in the copy: 1272GBPc
   |
 The copy cited: gov table1
```

</div>
<div markdown>

### A mistyped amount fails

Cite the table a tariff sheet, a rate page or a company rule came from (`@gov table1`) and it is kept as a copy. From then on **one wrong digit fails**, and a figure the copy states that no row uses is W120 — the other half of the same slip. A statute is pinned to the government's own text, on e-Gov or the eCFR, and an amendment names the rows to reread.

</div>
</div>

<div class="rc-row" markdown>
<div markdown>

```console
$ rulec verify rules/minimum_wage.rule --adapter python3 adapter.py
Compared 29 / matched 0 (0.000%)
Counterpart: payroll@2025-04

Affected 29 (100.000%)  amount +1,515
  table by_age row 2        4 records  difference +85 uniform  total +340
    Example: age=18, apprentice=false → rule hourly=1085 / legacy hourly=1000
```

</div>
<div markdown>

### How many records move, before it ships

`verify` against the implementation that runs today; `replay` and `diff` over what actually happened. Mismatches come back clustered by **the rows that matched**, with counts, amounts and an example.

</div>
</div>

<div class="rc-row rc-row--flip" markdown>
<div markdown>

```markdown
Source: gov table1 (sources/uk-nmw.md, sha256:bc45eedf7f908896)

> | Age group | Hourly rate |
> |---|---|
> | 21 and over | £12.71 |
> | 18 to 20 | £10.85 |

| # | age | → hourly (money[GBPc] / down(1GBPc)) |
|---|---|---|
| 3 | >=21 | 1271GBPc |
```

</div>
<div markdown>

### There is a page for the person who approves

`rulec doc` sets the copy it was transcribed from beside the rule's own table. The HTML page an approver tries a case on, and the article a help centre publishes, come from the same rule.

</div>
</div>

<div class="rc-row" markdown>
<div markdown>

```json
{"code": "E101",
 "where": {"line": 30, "table": "base_rate"},
 "witness": {"inputs": {"dest": "overseas", "size": "small", "weight": 1}},
 "fix": {"kind": "add_row", "text": "| overseas | small | 1lb | 6USD |"}}
```

</div>
<div markdown>

### An agent never has to read prose

Every command that reports findings has `--format json` (all but `source` and `import`), and the codes and the JSON shape **stay put while the wording improves**. Where there is no shell, `rulec mcp` serves the same commands as tools.

</div>
</div>

**Twelve targets** · **83 diagnostics** · **47 rules checked, generated and run on every commit — 21 transcribed from a published source** · **no dependencies, no runtime** · **one binary** · **the checks are offline**

---

## Where to start, by what you have

What you already have decides the first move. None of the three changes anything that
runs today, and the second is the one to start with when there is code already: nothing
is deployed, and what comes back is a match rate and the disagreements, clustered.

| you have | the first move | the command |
|---|---|---|
| **a spreadsheet, a published policy or a statute** | Transcribe it into a `.rule` and check it. From a workbook, a first draft is read straight out of the file, with every guess marked. From a statute, each table cites its section (`@osha "§1910.157"`) and is held to a copy of the text fetched from the statute database it names — the eCFR for the US federal regulations, e-Gov for a Japanese law; from a tariff sheet, a rate page or a company rule, it cites the table (`@gov table1`) and **a transcribed amount that disagrees with the copy fails**. No data and no old implementation are needed: a gap or a contradiction comes back with the input that causes it | `rulec import xlsx`, then `rulec check` — [What it proves](checks.md) |
| **an implementation that runs today** | Hand the existing function to the agent. It transcribes it into a `.rule` and wraps the old code in a 20-to-30-line adapter whose shape rulec prints; `verify` streams the cases built from the rule's own boundaries through both and returns where they disagree, clustered by the rows that matched, with counts and an example. The code that runs today is not touched | `rulec verify` — [Compare and replay](compare.md#against-a-legacy-implementation) |
| **past records** | Validate the records, then replay the rule over them. For a change, how many records move and by how much comes out before it ships — and **which inputs move at all** needs no records, only the two versions | `rulec fixtures lint`, then `rulec replay` / `rulec diff` — [Compare and replay](compare.md#against-what-actually-happened) |


## What this is — a language for table-shaped rules, and a harness for an agent

A great many business rules **can be written as a table**. Some already are — a shipping
tariff, a fee schedule, a price list. Others live in prose, or in what people just know, and
turn out to need **a table and a little arithmetic** once someone writes them down: which
discounts apply, whether a return is accepted, which period a date falls in, what rank a set
of scores earns. rulec is aimed at both — at anything a table could hold, not only at what is
already in one.

Today that rule is in a spreadsheet, a published policy, a wiki page, or somebody's head, and
an engineer rewrites it as a chain of `if`s. That is the normal way it goes.

rulec is **a harness for handing that rewrite to an AI agent**. It has four sides.

| | |
|---|---|
| **The way in is a table** | What the agent copies the policy into is one table a person can read. Being readable by someone other than its author is what makes approval possible at all |
| **The guard is the checker** | If the copied table has a gap or a contradiction, it stops before anything runs, holding the exact input that causes it. There is no "probably fine" |
| **The way out is code** | Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy that nobody edits by hand. **A target outside those twelve — another language, a workflow engine's expressions, a spreadsheet formula — can be generated today by asking an agent**, with the comparison against the rule coming along ([other targets](backends.md)). For an agent that calls the rule rather than embeds it, the same function comes out as one MCP tool ([the rule as a tool](generate.md#the-rule-as-a-tool-for-an-agent)) |
| **There is something to hand a person** | A document to approve, a page to try a case on, and a diff saying how many records move and by how much. What the agent cannot decide on its own becomes a question for a human |

The agent's own instructions are in [For agents](agents.md), and the
[agent skill](https://github.com/i2y/rulec/tree/main/skills) built from them ships in the
repository — copy the `skills/rulec/` folder into your project's `.claude/skills/` and it
works as it stands ([how](install.md#the-agent-skill)). Where there is no shell, `rulec mcp` offers the same commands as tools ([how](install.md#the-mcp-server)). Every rulec command has `--format json`, so an agent never parses prose. Diagnostic codes are fixed symbols like `E101`: **the wording improves, the code and
the JSON shape do not**.

In one line: **for business rules that can be written as a table, a tool that lets an agent
run the whole loop itself — write it, prove it, fix it, generate it, and show a person what
changed.**

---

## A person, an agent, and rulec — who hands what to whom

There are two kinds of shape and nothing else. A **sheet** with a folded
corner is a thing that gets handed over — the table, the diagnosis, the
question, the answer, the code. A **card** with a coloured bar is whoever
makes it or takes it. Every arrow runs card → sheet or sheet → card, so
**who produces what, and who consumes it** is the geometry itself rather
than a caption.

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy functions generated from the proved table, and the impact known before you deploy](images/flow.svg?v=9abfb225#only-dark)

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift, Java, SQL, Wasm and NumPy functions generated from the proved table, and the impact known before you deploy](images/flow-light.svg?v=9abfb225#only-light)

Three colours, three paths: **grey** for what enters and leaves the whole
system, **indigo** for the loop between the agent and rulec, **amber** for
the detour through a person.

The indigo loop is the middle of it. Hand over the table, get back a
diagnosis — where, how to fix it, and an input that shows the problem —
fix it, hand it over again. **Round and round until it passes.** No person
appears in that loop at all.

A person is pulled onto the amber path only. Most tools pass "could not
tell" off as a pass, or fill it with a plausible default. rulec stops
there and turns it into a question with a real case in it — *"what is the
fee for a small parcel going overseas?"* What the person answers is an amount and a
rounding direction; they never see a line of code. The answer goes into
the table, and the indigo loop picks up again.

---

## What kind of rule is this language for

**A rule that decides one transaction, in one shot, from flat facts.** The facts
are not nested, and a sequence of same-shaped elements can be walked once
(`elements` and `fold`) where the number of them is not fixed. The answer it gives
back is one of four things — **an amount, a yes/no, a
class, an order**.

- **Tariffs and shipping fees** — an amount decided by destination × size × weight
- **Discounts and coupons** — whether one applies, how much it takes off, which of two
  is applied first
- **Rates and charges** — a percentage that changes with membership tier, payment
  method or contract type
- **Eligibility** — may this be returned, is it covered by the guarantee, can this
  application be accepted
- **Classification** — which period does this date fall in, which size band, which
  priority
- **Routing** — which warehouse ships it, which desk handles it
- **Statutory provisions** — a tax table, the reduced rate that takes precedence over it, a proviso, a provision applied to another case

### Money is not required, and a statute has this shape too

A rule where no amount ever appears, and a tax table like Schedule 1 of the Stamp Tax Act
with its reduced rates, provisos and cross-references stacked on top, are both written the
same way. Whether yours is one of them is on [Does your rule fit](fit.md).


### The one constraint: a cell sees only its own column

A cell holds a condition on **the value in that column and nothing else**. `<=2000g` is
about the weight; `north_america` is about the destination. **No cell can span two columns** —
there is no way to write `weight × 10 > order total`. If you need that, name it first
with a `derive` or a `define` and make it a column of its own.

It looks restrictive, and it is what holds everything else up. Because every cell only
narrows its own column, **a row is a box** — a rectangle made of one interval per axis.
Boxes can be compared exactly: whether a gap is left between them, whether any two
overlap. Let a cell relate two columns and a row becomes an arbitrary shape, at which
point "is there a gap?" has no general answer.

That is where the boundary of this tool is drawn.

### Something complicated is written by stacking tables

Because a cell sees only its own column, **tables stack as deep as you like**: what one
table decides becomes a column of the next, and the rows that fired come back one per
table however deep the stack goes.

A table holds the branching and nothing else; the arithmetic lives in `derive`, `define`
and `result`. Both are on [Write a table (.rule)](tour.md#something-complicated-is-written-by-stacking-tables).


### What money buys you on top

This tool started life on a shipping tariff, so the machinery around amounts is the
thickest part of it.

- Units (USD / 円 / g / cm) and the **tax flag (inclusive / exclusive) are part of the type**.
  Mix them and it stops at compile time.
- A numeric output must declare `round`. When one is missing, the message shows the gap
  **in yen** — "the rounding mode moves this by up to 9 yen" — before it asks.
- When you ask what a change does, the money that moves is reported next to the number
  of records.

In a rule that returns no amount, those three simply go unused. Everything else works
the same.

### Why this shape

Because these five hold at once, and every feature of the tool is paid for by one of
them.

| the property | what the tool spends on it |
|---|---|
| **Being wrong costs something** | nothing is approximated to make it pass; what cannot be proved stops. If the cost is money, the three above apply; if it is "wrongly refused, or wrongly let through", the gap and overlap checks are what apply |
| **The right answer is written down elsewhere** | a tariff, a set of terms, a contract. The work is not designing something from nothing, it is **transcribing** — which is why the first thing the tool is worth is the gap showing up as you copy it across |
| **Conditions interlock** | destination × size × weight, kind × period × tier. It does not fit in one `if`, and no one can confirm by eye that the combinations are covered |
| **It is revised on a date** | a tariff revision, a campaign window, a change of terms. So you can point at two versions and get, before you deploy, how many records change and by how much |
| **The writer is not the decider** | the amount, the rounding direction and where a class begins are all business decisions. So there is a rendering for the person who approves, and a checker that turns what it cannot decide into a question with a real case in it |

### What the language is trying to be

**One file doing three jobs.** The same `.rule` is the **specification** a person
approves, the **subject** the checker proves things about, and the **source** the
generated code comes from. The moment those become three files, one of them rots — and
it is almost always the specification.

Whether a rule of yours is one of these — the five questions, the shape of apportionment
that fits and the one that does not, and what the language is not for — has a page of its
own: [Does your rule fit](fit.md).

### What else is out there

**Decision tables are not new, and neither is checking them.** Tables proved free of gaps and
overlaps go back to the formal methods of the 1990s, and most rules engines check a table as
it is edited. An honest look at the neighbours — read off their published material, not from
first-hand use.

| | What it is | How rulec differs |
|---|---|---|
| **Tables in formal methods** — SCR (US Naval Research Laboratory), PVS tables, the [Tabular Expression Toolbox](https://www.mathworks.com/matlabcentral/fileexchange/28812-tabular-expression-toolbox) | Requirements written as tables whose coverage and disjointness are proved, with the case that breaks them when they fail ([SCR](http://www.cs.toronto.edu/~chechik/courses99/ece450/1997heitmeyer-compass97.pdf), [PVS](https://pvs.csl.sri.com/doc/tacas97.pdf)) | The two properties are theirs, and so is refusing a table that lacks them. They were aimed at control software and a theorem prover; rulec aims them at tariffs, terms and statutes, with money, units and rounding, and at code in the languages applications are written in |
| **DMN** (the OMG standard) and its implementations — Apache KIE / Drools, Camunda, jDMN, Kogito, Trisotech | The industry standard for decision tables, with hit policies, and static gap/overlap analysis in several implementations ([Drools DMN](https://kie.apache.org/drools/dmn/), [dmn-check](https://github.com/red6/dmn-check)). [jDMN](https://github.com/goldmansachs/jdmn) generates Java | A DMN cell holds a FEEL expression, so completeness is hard in general and the analyses work over a subset. rulec keeps **a cell to its own column** — no cell spans two — which is what puts completeness and overlap on the decidable side. Units and tax class as types, mandatory rounding, an int64 proof, generating into several languages, and comparison against a legacy implementation are all outside DMN |
| **Rules engines** — Drools DRL, IBM ODM, Pega, SAP BRFplus, [GoRules / ZEN](https://github.com/gorules/zen), OpenRules, OpenL Tablets | Evaluate rules at runtime through a library or a service. Most of them also check a decision table for gaps and overlaps as it is edited ([IBM ODM](https://www.ibm.com/docs/en/odm/8.10.0?topic=tables-decision-table-errors-warnings)), and SAP BRFplus can be set to refuse to activate one that fails. IBM ODM can take a column's domain from an XML Schema, so a value added there shows up in the table as a gap | rulec **ships no engine**. What comes out is a dependency-free ordinary function, and rulec is not present at runtime |
| **Corticon** (Progress, commercial) | Rulesheets with a [conflict checker and a completeness checker](https://docs.progress.com/bundle/corticon-js-rule-modeling/page/The-conflict-checker.html). The closest in ambition | Commercial, with its own runtime. rulec hands over plain source and stops there — and carries the comparison side (verify / replay / diff) itself |
| **[LF-ET](https://www.lohrfink.de/en/solutions/lf-et/)** (Lohrfink, commercial) | Checks a decision table for completeness, redundancy and contradiction, and generates code from it in many languages, COBOL and ABAP among them, with no engine at run time | The closest in what comes out. Its conditions are named expressions written in the target language, and the checks are over combinations of their outcomes; a rulec cell is a range or a set on its own column, so the checks reach the values themselves, and units, rounding and int64 come with them |
| **[Catala](https://github.com/CatalaLang/catala)** (Inria) | A language for writing statute law as a program, correctness-first, compiling to several languages. A [proof plugin](https://github.com/CatalaLang/catala/blob/master/compiler/verification/verification.mld) asks Z3 whether a definition can come out empty or two exceptions can apply at once, and returns the case; it warns and does not stop the build | The closest relative in spirit. Different in shape: Catala mirrors the structure of legal text (defaults and exceptions), not decision tables. rulec has exceptions that take precedence over a main rule, provisos and provisions applied to another case too, but its unit stays the table, with gaps and overlaps decided by rectangle arithmetic. Catala computes in arbitrary precision and rounds money to the cent on its own; rulec holds every value to int64 and asks for the rounding, so every language it generates gives the same answer. Unit types and matching a legacy implementation are not Catala's |
| **API contract checks** — [`buf breaking`](https://buf.build/docs/breaking/rules/), [oasdiff](https://www.oasdiff.com/) | Check that a change to a `.proto` or an OpenAPI document is compatible on the wire | They leave the decisions a contract feeds alone, by design: `buf breaking` does not read custom options such as Protovalidate's rules, and to both of them a value added to a request enum is not a breaking change. rulec holds the contract to the rule's inputs (E032, E033, E122), so a change the wire accepts and no row handles fails in CI |
| **[Morphir](https://github.com/finos/morphir)** (FINOS) | Model business logic once in an IR and emit it to many targets | Broad by design; checking a decision table for completeness is not what it is for |

**Each piece exists somewhere already** — gap and overlap proofs since SCR, checking as you
edit in most rules engines, the case that breaks a definition in Catala, checked tables
generated into many languages in LF-ET, the effect of a revision on past records in the
simulations rules engines run. **Two things we have not found anywhere else.** The checks
hand over their evidence: `rulec certificate` prints what the proofs rest on, a
dependency-free Python file and a program built from a Lean development re-check it, and the
Lean development proves that the checks imply the claims — the pattern of a certifying
algorithm, applied to a business table. And the contract an API is called through, with the
ranges, counts, required fields and enum values its validation states, is held to the rule's
inputs in the contract's own CI, so a change is checked for what it does to the decision, not
only to the wire. A rules engine that takes a column's domain from an XML Schema sees an added
enum value as a gap too, but inside the engine. Everything else is the assembly: cells narrow
enough that gaps and overlaps are exactly decidable, a witness every time, units and rounding
in the types, twelve languages with zero dependencies, comparison against what runs today —
and **all of it drivable by an agent through `--format json` alone**, because an agent is the
first user it is built for.

---

## The gap shows up the moment you transcribe

This is the first thing the tool is worth. Transcribe a published
tariff by destination and size, and leave one of the pairs out:

```
error[E101]: Completeness gap: some input matches no row
  --> rules/parcel.rule:30 table base_rate
   |
30 | table base_rate
   |       ^^^^^^^^^ the input space is not fully covered
   |
 An input that matches no row: dest = overseas, size = small, weight = 1lb
 hint: add a row that matches this input.
```

**No legacy implementation and no historical data required.** Run the
spreadsheet you already have through `rulec import xlsx` for a first
draft, then `rulec check`, and the holes and the overlaps start coming
out — each with a concrete input that exhibits it.

---

## What gets proved

<div class="rc-overview" markdown>
![One computation decides three defects: a gap (E101) is a stretch no row covers, an overlap (E105) is a stretch two rows both cover, and an unreachable row (E102) is one whose whole stretch the earlier rows take first. Same table in all three; one thing changed](images/checks.svg?v=9abfb225#only-dark)
![One computation decides three defects: a gap (E101) is a stretch no row covers, an overlap (E105) is a stretch two rows both cover, and an unreachable row (E102) is one whose whole stretch the earlier rows take first. Same table in all three; one thing changed](images/checks-light.svg?v=9abfb225#only-light)
</div>

Seven things are settled before anything is generated. **Five are proved statically** —
every input matches some row, no input matches two, no row matches nothing, units are never
confused, every intermediate fits in int64. **One is a declaration that has to be there** —
how fractions are settled. **One is run** — every worked example holds. If any of the seven
cannot be shown, nothing is generated.

**What is *not* proved matters just as much**: that the table matches reality, that the
generated code answers like the table, the row pairs the overlap proof could not reach, and
that the checker itself is right. All four, with where each layer stops, are on
[What it proves](checks.md) and [How it is checked](assurance.md).

## Then: is it the same as what we run today?

Where an implementation already runs, a 20-to-30-line adapter streams the generated vectors
through it and a match rate comes back with the disagreements, clustered. For a revision,
`rulec diff` answers **which inputs get a different answer** with no records at all, and
**how many of yours move** when you have them. [Compare and replay](compare.md).


## Who it is for

**An agent is the first user.** Writing a `.rule` from a policy
document, a spreadsheet or an old implementation, fixing what the
checks report, generating the code and showing the impact — that whole
loop is meant to run from `--help`, the diagnostics and their JSON,
with nobody to ask.

Two roles stay with people. **Someone approves the table** — amounts,
rounding directions, and which of two readings is right are business
decisions, and the checks do not make them; they turn what cannot be
decided into a question with a concrete case in it. And **someone owns
the application** the generated function is called from.

[The agent's procedure](agents.md){ .md-button .md-button--primary }
