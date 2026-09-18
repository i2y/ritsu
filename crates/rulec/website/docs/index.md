---
title: "Write the table. Ship the proof."
hide:
  - navigation
  - toc
---

<div class="rc-hero" markdown>
<img class="rc-hero__mark" src="images/mark.svg#only-dark" alt="">
<img class="rc-hero__mark" src="images/mark-light.svg#only-light" alt="">

# rulec

<p class="rc-hero__tag">Write the table. Ship the proof.</p>

<p class="rc-hero__lede">
<strong>A harness for an agent turning table-shaped business rules into
code</strong>.
</p>

<p class="rc-hero__lede">
A business rule — a shipping tariff, a coupon policy, an eligibility
test — is written as one table a domain expert can read; rulec proves the
table has no gaps, no contradictions and no dead rows, and then
generates ordinary Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL with no runtime to install.
<strong>The proof happens before the code exists</strong>: a rule that
cannot be proved does not generate.
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
![Write the table. rulec turns each row into a box in the input space and proves by computation that the boxes leave no gap and no overlap. If there is a gap, back comes the input that falls through it (Destination = Overseas, Weight = 2001g): add the row and run again — only what the fee is takes a person. Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL](images/overview.svg?v=7709cde9#only-dark)
![Write the table. rulec turns each row into a box in the input space and proves by computation that the boxes leave no gap and no overlap. If there is a gap, back comes the input that falls through it (Destination = Overseas, Weight = 2001g): add the row and run again — only what the fee is takes a person. Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL](images/overview-light.svg?v=7709cde9#only-light)
</div>

That is the whole of it in one picture. A table goes in; rulec turns each row into a box,
and only a table whose boxes leave no gap and no overlap comes out as code. The picture
shows a gap, but **the same computation decides overlaps and rows nothing reaches** — the
three are set side by side further down.


---

## Where to start, by what you have

What you already have decides the first move. None of the three changes anything that
runs today, and the second is the one to start with when there is code already: nothing
is deployed, and what comes back is a match rate and the disagreements, clustered.

| you have | the first move | the command |
|---|---|---|
| **a spreadsheet or a published policy** | Transcribe it into a `.rule` and check it. From a workbook, a first draft is read straight out of the file, with every guess marked. No data and no old implementation are needed: a gap or a contradiction comes back with the input that causes it | `rulec import xlsx`, then `rulec check` — [What it proves](checks.md) |
| **an implementation that runs today** | Hand the existing function to the agent. It transcribes it into a `.rule` and wraps the old code in a 20-to-30-line adapter whose shape rulec prints; `verify` streams the cases built from the rule's own boundaries through both and returns where they disagree, clustered by the rows that matched, with counts and an example. The code that runs today is not touched | `rulec verify` — [Compare and replay](compare.md#against-a-legacy-implementation) |
| **past records** | Validate the records, then replay the rule over them. For a change, how many records move and by how much comes out before it ships | `rulec fixtures lint`, then `rulec replay` / `rulec diff` — [Compare and replay](compare.md#against-what-actually-happened) |


## What this is — a harness for an agent turning table-shaped rules into code

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
| **The way out is code** | Only a proved table generates, and what comes out is dependency-free Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL that nobody edits by hand. **A target outside those eight — another language, a workflow engine's expressions, a spreadsheet formula — can be generated today by asking an agent**, with the comparison against the rule coming along ([other targets](backends.md)). For an agent that calls the rule rather than embeds it, the same function comes out as one MCP tool ([the rule as a tool](generate.md#the-rule-as-a-tool-for-an-agent)) |
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

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL functions generated from the proved table, and the impact known before you deploy](images/flow.svg?v=7709cde9#only-dark)

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come Python, TypeScript, JavaScript, Rust, Ruby, Go, Swift and SQL functions generated from the proved table, and the impact known before you deploy](images/flow-light.svg?v=7709cde9#only-light)

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
fee to 山梨県 at size S60?"* What the person answers is an amount and a
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

### Money does not have to be involved

Here is a rule with no money in it anywhere. One date goes in, one class comes out, and
it passes the checks as written.

```rule
rule 期間区分(period) v1

enum 期間(kind) = 改定前(before) | 春季(spring) | 通常(normal) | 年末(year_end)

inputs
  注文日(order_date) : date  range >=2026-01-01 <=2026-12-31

outputs
  区分(kind) : 期間

table 期間判定(pick)
policy unique
| 注文日                    | -> 区分(kind) : 期間 |
| <=2026-03-31              | 改定前               |
| >=2026-04-01 <=2026-06-30 | 春季                 |
| >=2026-07-01 <=2026-11-30 | 通常                 |
| >=2026-12-01              | 年末                 |
```

What decides it is the **shape of the decision**, not what the values happen to be. So
this is not "a tool for shipping fees" and not "a tool for e-commerce".

### The one constraint: a cell sees only its own column

A cell holds a condition on **the value in that column and nothing else**. `<=2000g` is
about the weight; `遠隔地` is about the destination. **No cell can span two columns** —
there is no way to write `weight × 10 > order total`. If you need that, name it first
with a `derive` or a `define` and make it a column of its own.

It looks restrictive, and it is what holds everything else up. Because every cell only
narrows its own column, **a row is a box** — a rectangle made of one interval per axis.
Boxes can be compared exactly: whether a gap is left between them, whether any two
overlap. Let a cell relate two columns and a row becomes an arbitrary shape, at which
point "is there a gap?" has no general answer.

That is where the boundary of this tool is drawn.

### So how is something complicated written — tables stack

Because a cell can only see its own column, **tables stack as deep as you like**. What one
table produces is written as a column of the next.

<div class="rc-overview" markdown>
![What one table produces is a column of the next: band_of turns the distance and whether the flight is intra-EU into a band, and amount turns that band into the compensation. Not every table is in the chain — reduction reads the rule's inputs directly — and result puts the two together](images/stack.svg?v=7709cde9#only-dark)
![What one table produces is a column of the next: band_of turns the distance and whether the flight is intra-EU into a band, and amount turns that band into the compensation. Not every table is in the chain — reduction reads the rule's inputs directly — and result puts the two together](images/stack-light.svg?v=7709cde9#only-light)
</div>

What to look at is **the word that appears twice**. `band` leaves the first table and arrives
as a column of the second. Not every table is in the chain: `reduction` reads the rule's
inputs directly, because Article 7(2) restates the distance conditions rather than referring
back to them.

Depth costs no visibility. When a check fails it names the row that fired in each table.

```
Fired rows: table band_of row 4 / table amount row 3 / table reduction row 8
```

Four things matter when stacking.

| | |
|---|---|
| **A table's output is a column of any later table** | There is no limit on the depth; only the check's budget stops it, at E109 |
| **One table may produce several output columns** | `送料表` above produces `送料` and `倍率` at once |
| **A `derive` can be a column** | "Judge on the amount after the discount" becomes one column instead of one bare line of arithmetic |
| **Completeness is checked across the stack** | The second form of E102 is "the upstream table never emits that value" |

A rule that does this, and runs, is [Examples](examples.md) → "Three tables stacked, two
outputs returned".

### Where the arithmetic goes

A table holds **the branching and nothing else**. Arithmetic lives in three places outside it.

| | what it may hold | can it be a column? |
|---|---|---|
| `derive` | a linear combination of inputs — `+`, `-`, multiplication by a constant | **yes**, and it stays a quantity |
| `define` | a boolean (two shapes), or a computed intermediate value | a boolean or an enum one can |
| `result` | `+ - * /`, parentheses, `min` and `max`, and the five rounding modes as functions | — |

- **Division is by a constant only.** Dividing by a variable stops at E115. Dividing money by
  a money constant — `税込金額 ÷ 100円` — cancels the unit and leaves a `number`.
- Multiplication by a rate is allowed: `基本送料 × 負担率`. The rate stays a rate to the end,
  and rounding happens exactly once.
- **There is no loop and no recursion** (bar a `fold`, which walks a sequence once), and no date arithmetic — comparison and range only.
- Everything is an integer. No floating point appears anywhere.

**When a rule has more than one output**, `result` assembles the first one and nothing else
(E015); the rest are taken from a `define` of the same name as the output. A second `result`
line stops at E016.

**Most numbers you return carry a unit.** The numeric types are **quantity (mass, length),
money and rate**, plus `number` for the ones that carry none — a count of things, a
number of days, a score. Numbers with a unit and numbers without do not mix, and the
only way a unit disappears is dividing money by money: "one point per 100 yen" is a
`number`.

### What money buys you on top

This tool started life on a shipping tariff, so the machinery around amounts is the
thickest part of it.

- Units (円 / g / cm) and the **tax flag (inclusive / exclusive) are part of the type**.
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

**Decision tables are not new.** An honest look at the neighbours — read off their published
material, not from first-hand use.

| | What it is | How rulec differs |
|---|---|---|
| **DMN** (the OMG standard) and its implementations — Apache KIE / Drools, Camunda, jDMN, Kogito | The industry standard for decision tables, with hit policies, and static gap/overlap analysis in some implementations ([Drools DMN](https://kie.apache.org/drools/dmn/), [dmn-check](https://github.com/red6/dmn-check)). [jDMN](https://github.com/goldmansachs/jdmn) generates Java | A DMN cell holds a FEEL expression, so completeness is hard in general and the analyses work over a subset. rulec keeps **a cell to its own column** — no cell spans two — which is what puts completeness and overlap on the decidable side. Units and tax class as types, mandatory rounding, an int64 proof, generating into several languages, and comparison against a legacy implementation are all outside DMN |
| **Rules engines** — Drools DRL, IBM ODM, [GoRules / ZEN](https://github.com/gorules/zen), OpenRules, OpenL Tablets | Evaluate rules at runtime through a library or a service | rulec **ships no engine**. What comes out is a dependency-free ordinary function, and rulec is not present at runtime |
| **Corticon** (Progress, commercial) | Rulesheets with a [conflict checker and a completeness checker](https://docs.progress.com/bundle/corticon-js-rule-modeling/page/The-conflict-checker.html). The closest in ambition | Commercial, with its own runtime. rulec hands over plain source and stops there — and carries the comparison side (verify / replay / diff) itself |
| **[Catala](https://github.com/CatalaLang/catala)** (Inria) | A language for writing statute law as a program, correctness-first, compiling to several languages | The closest relative in spirit. Different in shape: Catala mirrors the structure of legal text (defaults and exceptions), not decision tables, and has neither unit types nor a story for matching a legacy implementation |
| **[Morphir](https://github.com/finos/morphir)** (FINOS) | Model business logic once in an IR and emit it to many targets | Broad by design; checking a decision table for completeness is not what it is for |

**Where rulec sits is the combination**: cells narrow enough that gaps and overlaps are
exactly decidable, a witness (the input itself) attached every time, units and rounding held
by types and declarations, eight languages out with zero dependencies, comparison against the
old implementation and against past records — and **all of it drivable by an agent through
`--format json` alone**. Each piece exists somewhere already. The assembly, and treating an
agent as the first user, is the position.

---

## The gap shows up the moment you transcribe

This is the first thing the tool is worth. Transcribe a published
tariff, fold 47 prefectures into six groups, and leave one of them out:

```
error[E101]: Completeness gap: some input matches no row
  --> rules/ゆうパック運賃.rule:34 table 運賃表
   |
34 | table 運賃表(fee_table)
   |       ^^^^^^ the input space is not fully covered
   |
 An input that matches no row: あて先 = 山梨県, サイズ = S60
 hint: add a row that matches this input.
```

**No legacy implementation and no historical data required.** Run the
spreadsheet you already have through `rulec import xlsx` for a first
draft, then `rulec check`, and the holes and the overlaps start coming
out — each with a concrete input that exhibits it.

---

## What gets proved

The first three are **one computation**. Lay the rectangle each row covers over the input
space, and ask whether anything is left uncovered, whether two rows cover the same stretch,
and whether an earlier row takes a later row's stretch first. Same table in all three; one
thing changed.

<div class="rc-overview" markdown>
![One computation decides three defects: a gap (E101) is a stretch no row covers, an overlap (E105) is a stretch two rows both cover, and an unreachable row (E102) is one whose whole stretch the earlier rows take first. Same table in all three; one thing changed](images/checks.svg?v=7709cde9#only-dark)
![One computation decides three defects: a gap (E101) is a stretch no row covers, an overlap (E105) is a stretch two rows both cover, and an unreachable row (E102) is one whose whole stretch the earlier rows take first. Same table in all three; one thing changed](images/checks-light.svg?v=7709cde9#only-light)
</div>

The other four — units, rounding, overflow, examples — are not rectangle arithmetic. They
are held by the types and the declarations.

| | |
|---|---|
| **Completeness** | every input matches some row, or you get the input that does not |
| **Overlap** | under `policy unique`, two rows never match the same input. Under `policy first`, structural shadowing is told apart from the pairs that need a human decision |
| **Dead rows** | a row nothing can reach, whether because earlier rows cover it or because the upstream table never emits the value it names |
| **Units** | yen and grams do not add. Tax-inclusive and tax-exclusive are different types |
| **Rounding** | a numeric output must declare how fractions are settled — and the message shows, in yen, how much the choice moves |
| **Overflow** | every intermediate value is proved to fit in int64, from the declared ranges |
| **Examples** | every example runs, and a failure names the rows that fired |

Nothing is approximated. When a check cannot prove something, it says
so rather than passing.

### What is *not* proved

Worth saying in the same breath.

1. **That the table matches reality.** Transcribe the tariff wrong and
   everything stays green. What gets proved is what can be said about
   the table you wrote.
2. **That the generated code answers like the table.** That is a
   **test**, not a proof: vectors built from the boundaries are fed to
   the reference evaluator and to every generated language and compared
   byte for byte. Strong evidence, not a proof of equivalence.
3. **Row pairs the overlap proof could not reach.** When a `unique`
   table has two rows and neither an input that hits both nor a proof
   that none exists can be constructed, **W114 names the pair and the
   obligation moves to a runtime guard**. That one spot has no static
   proof — instead of silently picking a row, the generated code raises.

[What it proves, in detail](checks.md){ .md-button }

---

## Then: is it the same as what we run today?

Two commands turn "deploy and watch the numbers" into "look before
deploying".

```console
$ rulec verify rules/送料.rule --adapter python3 adapter.py
Compared 207 / matched 182 (87.923%)

Affected 25 (12.077%)  amount -250
  table サイズ判定 row 1 / table 運賃表 row 36    7 records  difference -10 uniform  total -70
    Example: あて先=沖縄県, 三辺合計=1, 重量=1 → rule 運賃=1450 / legacy 運賃=1460
```

`verify` compares the rule against a legacy implementation, and `diff`
compares two versions of the rule over real past records and reports
**how many change and by how much**. Mismatches are clustered by the
rows that fired, with counts, amount differences and a witness; a
cluster whose differences are all smaller than the output's rounding
grid is flagged as a rounding convention rather than a disagreement.

[Compare and replay](compare.md){ .md-button }

---

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
