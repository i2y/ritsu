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

## What kind of rule is this language for

rulec is aimed at decisions where **money moves, the source of truth is a document
outside the code, the rule is revised on a date, and a person signs it off**.

- **Tariffs and shipping fees** — an amount decided by a combination of destination,
  size and weight
- **Discounts and coupons** — whether one applies, how much it takes off, which of two
  is applied first
- **Rates and charges** — a percentage that changes with membership tier, payment
  method or contract type
- **Eligibility and classification** — may this be returned, which period does this date
  fall in, which band does this land in

All seven rules in the corpus are of that kind, transcribed from Japan Post's tariff
table, Yamato's size bands and Rakuten's and Yahoo's coupon terms. Their inputs are one
to ten flat values; their outputs are one of four things — **an amount, a yes/no, a
class, an order** — and every one of them decides a single transaction in a single shot.

### Why those four

Because five properties hold at once, and every feature of the tool is paid for by one
of them.

| the property | what the tool spends on it |
|---|---|
| **Money moves** | units (円 / g / cm) and the tax flag (incl./excl.) are part of the type, and a numeric output that declares no `round` does not compile. When one is missing, the message shows the gap **in yen** — "the rounding mode moves this by up to 9 yen" — before it asks |
| **The source is outside** | the work is not designing something from nothing, it is **transcribing** a tariff or a set of terms. That is why the first thing the tool is worth is "the gap shows up the moment you transcribe it", and why `doc` writes no sentence that does not trace back to the source or to a check |
| **Conditions interlock** | destination × size × weight, kind × period × tier. It does not fit in one `if`, and no one can confirm by eye that the combinations are covered. Hence a decision table, and hence a completeness check that means something |
| **It is revised on a date** | a tariff revision, a campaign window, a change of terms. Hence versions (`送料@v3`), hence `--diff-base` showing only what is newly reported, hence `diff` putting a number on the impact using real past records |
| **The writer is not the decider** | the amount and the rounding direction are business decisions. Hence `doc`, and hence a checker that turns what it cannot decide into a question with a real case in it |

### What the language is trying to be

**One file doing three jobs.** The same `.rule` is the **specification** a person
approves, the **subject** the checker proves things about, and the **source** the
generated code comes from. The moment those become three files, one of them rots — and
it is almost always the specification.

### Does your rule fit

If all five are **yes**, it fits.

1. Is the **number of inputs fixed** (not a list of variable length)?
2. Are the inputs **flat values** (you can pass the prefecture itself, not
   `order.destination.prefecture`)?
3. Is it **one decision** (iteration and ordering can live in the caller)?
4. Does the **same input always give the same answer** ("today" and the stock level are
   arguments too)?
5. Does **a person approve** the answer, or is the rule **revised on a date**?

If any of 1–4 is no, it does not fit structurally. If only 5 is no, it will work, but
the tool is more than you need.

### What it is not for

- **Workflows** — several steps, carrying state
- **Judgements about a collection** — "any line item is refrigerated", "three or more
  items in the cart". Flatten those at the boundary and pass the scalar in
- **Pattern matching on strings** — `string` has equality and set membership, no prefix
  match and no regular expressions
- **Scoring, optimisation, machine learning** — what can be proved here is which row
  fires, not whether a weight is right
- **Proration** — it has not once come up in a transcription, so whether this choice of
  granularity survives it is **untested** (DESIGN §15-7)

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
