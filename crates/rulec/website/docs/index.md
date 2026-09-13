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
A business rule — a shipping tariff, a coupon policy, an eligibility
test — written as one table a domain expert can read. rulec proves the
table has no gaps, no contradictions and no dead rows, and then
generates ordinary Python and Go with no runtime to install.
<strong>The proof happens before the code exists</strong>: a rule that
cannot be proved does not generate.
</p>

<div class="rc-hero__cta" markdown>
[Install](install.md){ .md-button .md-button--primary }
[Write a table](tour.md){ .md-button }
[For agents](agents.md){ .md-button }
[GitHub](https://github.com/i2y/rulec){ .md-button }
</div>
</div>

## Table in, functions out

<div class="rc-flow" markdown>
<div markdown>
<p class="rc-flow__label">what you write</p>

```
table 運賃表(fee_table)
policy unique
| あて先      | サイズ | -> 運賃(fee) : money[円, incl_tax] |
| 近畿圏      | S60    | 990円                              |
| 近畿圏      | S80    | 1310円                             |
| not: 近畿圏 | S60    | 880円                              |
| not: 近畿圏 | S80    | 1200円                             |
```
</div>
<div class="rc-flow__arrow">→</div>
<div markdown>
<p class="rc-flow__label">what you get</p>

```python
def fee_demo(dest: Prefecture, girth: Cm) -> YenInclTax:
    """Rule 送料例 v1. Each branch corresponds 1:1 to a row."""
    if not _isinstance(dest, Prefecture):
        raise RuleInputError(...)
    ...
    if dest in KINKI and サイズ == SizeClass.S60:  # row 1
        運賃 = 990
    ...
    return _round_up(運賃, 10)
```
</div>
</div>

The same table also becomes a Go package. Both are plain functions: no
engine, no configuration, no dependency beyond the standard library.

---

## What this is — a harness for an agent turning table-shaped rules into code

A great many business rules **are already written as tables**. A shipping tariff, a fee
schedule, which discounts apply, whether a return is accepted, which period a date falls in.
The table lives in a spreadsheet, a published policy, or a wiki page, and an engineer rewrites
it as a chain of `if`s. That is the normal way it goes.

rulec is **a harness for handing that rewrite to an AI agent** — a workbench, with guards on
it. It has four sides.

| | |
|---|---|
| **The way in is a table** | What the agent copies the policy into is one table a person can read. Being readable by someone other than its author is what makes approval possible at all |
| **The guard is the checker** | If the copied table has a gap or a contradiction, it stops before anything runs, holding the exact input that causes it. There is no "probably fine" |
| **The way out is code** | Only a proved table generates, and what comes out is dependency-free Python and Go that nobody edits by hand |
| **There is something to hand a person** | A document to approve, and a diff saying how many records move and by how much. What the agent cannot decide on its own becomes a question for a human |

The agent's own instructions are in [For agents](agents.md), and the
[agent skill](https://github.com/i2y/rulec/tree/main/skills) built from them ships in the
repository — copy `skills/rulec/` into your project's `.claude/skills/` and it works as it
stands. Every rulec command has `--format json`, so an agent never parses prose. Diagnostic codes are fixed symbols like `E101`: **the wording improves, the code and
the JSON shape do not**.

In one line: **for business rules that can be written as a table, a tool that lets an agent
run the whole loop itself — write it, prove it, fix it, generate it, and show a person what
changed.**

---

## What kind of rule is this language for

**A rule that decides one transaction, in one shot, from a fixed number of flat
facts.** The answer it gives back is one of four things — **an amount, a yes/no, a
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

```
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

What decides it is the **shape of the decision**, not what the values happen to be. A
cell tests its own column and nothing else, which makes a row a box, which makes gaps
and overlaps exactly decidable — that is where the boundary is. So this is not "a tool
for shipping fees" and not "a tool for e-commerce".

**Most numbers you return carry a unit.** The numeric types are **quantity (g, cm),
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

### Does your rule fit

If all five are **yes**, it fits. Whether money is involved is not one of them.

1. Is the **number of inputs fixed** (not a list of variable length)?
2. Are the inputs **flat values** (you can pass the prefecture itself, not
   `order.destination.prefecture`)?
3. Is it **one decision** (iteration and ordering can live in the caller)?
4. Does the **same input always give the same answer** ("today" and the stock level are
   arguments too)?
5. Does **a person approve** the answer, or is the rule **revised on a date**?

If any of 1–4 is no, it does not fit structurally. If only 5 is no, it will work, but
the tool is more than you need.

### Apportionment depends on how you apportion

Spreading a discount across the lines of an order can or cannot be written, depending on
how the split is decided.

**It can — when you fill each line in turn.** "Apply the discount to the lines in order,
up to each line's own value." Make the rule decide one line, and leave the loop to the
caller.

```
inputs
  明細定価(list)      : money[円, incl_tax]  range >=0円 <=100万円
  残り値引(remaining) : money[円, incl_tax]  range >=0円 <=100万円
  対象(eligible)      : bool

outputs
  充当額(applied) : money[円, incl_tax]  round down(1円)

define 充てられる額(cap) : money[円, incl_tax] = min(明細定価, 残り値引)

table 充当可否(applies)
policy unique
| 対象  | -> 充当(on) : money[円, incl_tax] |
| true  | 充てられる額                      |
| false | 0円                               |

result 充当額 = 充当
```

The caller walks the lines and subtracts from `残り値引`. Done this way **the total
always comes out exact** — nothing is over- or under-allocated (checked over 2,000
different sets of lines).

**It cannot — when you split by ratio.** "Apportion by each line's share of the list
price" needs `line ÷ total`, and **division is only allowed by a constant**:
`注文金額 ÷ 100円` is fine, `明細 ÷ 合計` is not. You can work around it by computing
the ratio in the caller and passing it in as a rate — a rate step goes as fine as you
declare it, `rate[step 0.1%]` and beyond — but then whether that ratio is right is no
longer something this tool says anything about.

### What it is not for

- **Workflows** — several steps, carrying state
- **Judgements about a collection itself** — "any line is refrigerated", "three or more
  items in the cart". Flatten those at the boundary and pass the scalar in
- **Pattern matching on strings** — `string` has equality and set membership, no prefix
  match and no regular expressions
- **Scoring, optimisation, machine learning** — what can be proved here is which row
  fires, not whether a weight is right

### What else is out there

**Decision tables are not new.** An honest look at the neighbours — none of which I have used,
so this is from their published material.

| | What it is | How rulec differs |
|---|---|---|
| **DMN** (the OMG standard) and its implementations — Apache KIE / Drools, Camunda, jDMN, Kogito | The industry standard for decision tables, with hit policies, and static gap/overlap analysis in some implementations ([Drools DMN](https://kie.apache.org/drools/dmn/), [dmn-check](https://github.com/red6/dmn-check)). [jDMN](https://github.com/goldmansachs/jdmn) generates Java | A DMN cell holds a FEEL expression, so completeness is hard in general and the analyses work over a subset. rulec restricts a cell to **a unary test on its own column**, which is what puts completeness and overlap on the decidable side. Units and tax class as types, mandatory rounding, an int64 proof, two target languages and comparison against a legacy implementation are all outside DMN |
| **Rules engines** — Drools DRL, IBM ODM, [GoRules / ZEN](https://github.com/gorules/zen), OpenRules, OpenL Tablets | Evaluate rules at runtime through a library or a service | rulec **ships no engine**. What comes out is a dependency-free ordinary function, and rulec is not present at runtime |
| **Corticon** (Progress, commercial) | Rulesheets with a [conflict checker and a completeness checker](https://docs.progress.com/bundle/corticon-js-rule-modeling/page/The-conflict-checker.html). The closest in ambition | Commercial, with its own runtime. rulec hands over plain source and stops there — and carries the comparison side (verify / replay / diff) itself |
| **[Catala](https://github.com/CatalaLang/catala)** (Inria) | A language for writing statute law as a program, correctness-first, compiling to several languages | The closest relative in spirit. Different in shape: Catala mirrors the structure of legal text (defaults and exceptions), not decision tables, and has neither unit types nor a story for matching a legacy implementation |
| **[Morphir](https://github.com/finos/morphir)** (FINOS) | Model business logic once in an IR and emit it to many targets | Broad by design; checking a decision table for completeness is not what it is for |

**Where rulec sits is the combination**: cells narrow enough that gaps and overlaps are
exactly decidable, a witness (the input itself) attached every time, units and rounding held
by types and declarations, two languages out with zero dependencies, comparison against the
old implementation and against past records — and **all of it drivable by an agent through
`--format json` alone**. Each piece exists somewhere already. The assembly, and treating an
agent as the first user, is the position.


---

## A person, an agent, and the tool

There are two kinds of shape and nothing else. A **sheet** with a folded
corner is a thing that gets handed over — the table, the diagnosis, the
question, the answer, the code. A **card** with a coloured bar is whoever
makes it or takes it. Every arrow runs card → sheet or sheet → card, so
**who produces what, and who consumes it** is the geometry itself rather
than a caption.

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come proved Python and Go, and the impact known before you deploy](images/flow.svg#only-dark)

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come proved Python and Go, and the impact known before you deploy](images/flow-light.svg#only-light)

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

**No legacy implementation and no historical data required.** Transcribe
the spreadsheet you already have, run `rulec check`, and the holes and
the overlaps start coming out — each with a concrete input that
exhibits it.

---

## What gets proved

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
