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

## A person, an agent, and the tool

The **in / out** rows inside each box are what that actor takes and what it
gives; each arrow is labelled with **the thing that moves** rather than with
a verb. The `rulec check` and `rulec gen` above two of them are the commands
that cause them — rulec never acts on its own, the agent runs it.

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come proved Python and Go, and the impact known before you deploy](images/flow.svg#only-dark)

![The agent writes the table, rulec proves it and returns what to fix, and only what cannot be decided goes to a person. Out come proved Python and Go, and the impact known before you deploy](images/flow-light.svg#only-light)

① → ② → ④ is the **loop**: the agent fixes what the diagnosis names and
goes back to ①, over and over until it is green. A person is pulled in
only at ③.

And **arrow ③ is the one that matters.** Most tools pass "could not tell"
off as a pass, or fill it with a plausible default. rulec stops there and
hands a person one concrete input — *"what is the fee to 山梨県 at size
S60?"* Phrased that way, the question comes back answered in one round
trip.

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
