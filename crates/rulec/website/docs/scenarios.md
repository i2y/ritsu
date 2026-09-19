# How to use it, by role

One tool, but **what you have in hand and what you want out** decide the path through it. Three readers, three paths. Start from the one closest to you.

| You are | What you have | What you want | Read |
|---|---|---|---|
| **implementing** a public rule (a statute, a published policy, a tariff) | the article, or the policy PDF | code that does what the article says, and that notices the amendment | [1. Implementing an existing public rule](#1-implementing-an-existing-public-rule) |
| **designing** a new rule (a new public rule, an internal rule, the terms of an online shop or a service) | the conditions, in prose or in your head | a table with no gap and no contradiction, and the pages that get it approved or published | [2. Designing a new rule](#2-designing-a-new-rule) |
| **implementing** from a finished rule | a `.rule` that passes check | code in your own language that answers exactly like the table | [3. Implementing from a new rule](#3-implementing-from-a-new-rule) |

The middle step is the same on every path: **nothing comes out of a table that does not pass `rulec check`** ([What it proves](checks.md)). Every command below is one of these:

```console
$ rulec --help
```

---

## 1. Implementing an existing public rule

You have a statute or a published policy and want code that does exactly what it says. The lead role here is the **source**: which row came from which article, held to a copy of the text, so that an amendment is noticed.

![An agent transcribes a statute or a published policy into a table (.rule), citing the article with @. rulec holds the table to the copy, proves it has no gap and no overlap, and generates nine languages. The approver compares the article and the table on the page rulec doc renders. When an amendment comes, rulec source outdated says so and the table is reread](images/scenario-existing.svg#only-dark)
![An agent transcribes a statute or a published policy into a table (.rule), citing the article with @. rulec holds the table to the copy, proves it has no gap and no overlap, and generates nine languages. The approver compares the article and the table on the page rulec doc renders. When an amendment comes, rulec source outdated says so and the table is reread](images/scenario-existing-light.svg#only-light)

### 1-1. Transcribe, citing the article

An agent or a person transcribes; what matters is that every table or row ends with **`@source article`**, so that where it came from stays with it. For a statute, `source` names the law's id on e-Gov (the Japanese government's statute database) and the date the text is read as of.

```rule
source 法 = law "342AC0000000023" asof 2026-04-01
source 措置法 = law "332AC0000000026" asof 2026-04-01

define 軽減期間(reduced) : bool = 作成日 <= 2027-03-31  @措置法 第91条

table 本則(base)  @法 別表第一
policy unique
         | 金額の記載あり | 契約金額         | -> 印紙税額(tax) : money[円] |
記載なし | false          | -                | 200円                        |
r3       | true           | >=1万円 <=10万円 | 200円                        |
```

The whole notation is in [Write a table](tour.md#where-it-was-transcribed-from-source-and-). A supplementary provision is cited as `@法 附則第3条`, an amending law's as `@法 附則（令和七年三月三一日法律第一三号）第3条`.

### 1-2. Fetch the copy and pin it

`check` never reads the network. The article's copy lives beside the rule, and its digest is written into the rule. Two commands:

```console
$ rulec source fetch rules/stamp_duty.rule
措置法: fetched 第91条 (sha256:85faf53f6f6e8196)
$ rulec source pin rules/stamp_duty.rule
措置法: pinned 1 fragments
```

`pin` writes one line under the `source`: `  第91条 sha256:85faf53f6f6e8196`. The copies go to `sources/law/<law id>@<date>/`; commit them.

### 1-3. Check

```console
$ rulec check rules/stamp_duty.rule
ok rules/stamp_duty.rule
```

When the article's side changes, `check` stops there. A copy that differs is **E038**, naming the definitions to reread:

```console
$ rulec check rules/stamp_duty.rule
error[E038]: Fragment `第91条` of source `措置法` has changed
  --> rules/stamp_duty.rule:7 source 措置法
  |
7 |   第91条 sha256:85faf53f6f6e8196
  |   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ pinned: sha256:85faf53f6f6e8196
  |
 The copy now: sha256:cac55089de86c8eb
 Definitions to reread: definition 軽減期間, table 軽減
```

A missing pin is E037, a missing copy E039; `rulec explain E038` explains any of them.

### 1-4. Show it to the approver

Comparing the article with the table is a person's job. `rulec doc` renders the page, quoting the cited article from the copy.

```console
$ rulec doc rules/stamp_duty.rule > stamp_duty.md
```

Under the table's heading, the page reads:

```markdown
Source: 措置法 第91条 (law 332AC0000000026, as of 2026-04-01; in force from 2026-04-01, as amended by Act No. 12 of 2026)

> 第九十一条
> 平成二十六年四月一日から令和九年三月三十一日までの間に作成される…
```

The approver compares the rows with that quotation and nothing else. How to read the page is in [Showing it to the person who approves](checks.md#showing-it-to-the-person-who-approves).

### 1-5. Generate and hold the code to the table

```console
$ rulec gen rules/stamp_duty.rule --out generated/
$ rulec test generated/
ok    stamp_duty_split (Python) 296 vectors
ok    stamp_duty_split (SQL) 296 vectors
…
```

The header of every generated file names the article, its date and its digest, so a reader of the code can tell which text it was made from:

```python
# Cites: 措置法 = law 332AC0000000026 asof 2026-04-01 (第91条 sha256:85faf53f6f6e8196)
```

### 1-6. Notice the amendment

Statutes get amended. `check` is held to the copy, so this is the one way to learn of an amendment; put it in a weekly CI job.

```console
$ rulec source outdated rules/pension_premium.rule
厚生年金保険法: the amendment enforced on 2027-09-01 changes 第20条; the version in force from that day needs a reread rule
  add `source 厚生年金保険法_20270901 = law "329AC0000000115" asof 2027-09-01` and transcribe the rows in force from that day from it
  第20条: - 六〇五、〇〇〇円以上
  第20条: + 六〇五、〇〇〇円以上六三五、〇〇〇円未満
  第20条: + 第三二級
  第20条: + 六五〇、〇〇〇円
  …
```

What changes, from when, and the `source` line to add. The day an amendment takes effect is written as a row condition on a date column (`月分 >=2027-09-01`), with the old rows and the new rows in the same table; a gap or an overlap between them stops `check`.

!!! note "Take the date as an input, and the amendment folds in"

    An amendment usually says "applies to documents made on or after the day it comes into force; earlier ones follow the old rule". The switch is not the day you run the code but the day the document was made, the wage paid, the month insured. Take that date as an input and write it into the row conditions, and the versions need no separate files.

Putting it in CI is on the [install page](install.md#in-ci). `outdated` exits 1 when something changes, so its output can become an issue as it is.

---

## 2. Designing a new rule

Shipping fees, coupon conditions, whether a return is accepted, an internal criterion, a new public rule: something still being decided that you want to settle as a table. The lead role here is the **check**: every gap and every contradiction comes back with a concrete input that shows it, so what you forgot to decide is visible before you decide.

![Someone designing a rule (shipping, coupons, returns, an internal criterion) writes it as a table (.rule). rulec check returns every gap and overlap with an input that shows it, until the table passes. From a passed table come the approver's page, the customer article and the impact of a revision](images/scenario-designing.svg#only-dark)
![Someone designing a rule (shipping, coupons, returns, an internal criterion) writes it as a table (.rule). rulec check returns every gap and overlap with an input that shows it, until the table passes. From a passed table come the approver's page, the customer article and the impact of a revision](images/scenario-designing-light.svg#only-light)

### 2-1. Write the table first

Conditions as columns, the answer as the last column. "Hokkaido and Okinawa, 1,200 yen up to 2 kg" becomes one row.

```rule
rule 送料(shipping_fee) v1
description "Standard delivery fee"

import std/都道府県
group 遠隔地 = 北海道, 沖縄県

inputs
  届け先(dest) : 都道府県
  重量(weight) : mass[g]  range >=1g <=40kg

outputs
  送料(fee) : money[円, incl_tax]  round up(10円)

table 基本送料(base)
policy unique
| 届け先      | 重量    | -> 送料(fee) : money[円, incl_tax] |
| 遠隔地      | <=2000g | 1200円                             |
| 遠隔地      | >2000g  | 1800円                             |
| not: 遠隔地 | <=2000g | 800円                              |
| not: 遠隔地 | >2000g  | 1100円                             |
```

If the rule already lives in a spreadsheet, a first draft can be made from it. Every guess is marked, so only the marked places need a look.

```console
$ rulec import csv tariff.csv --name 運賃 > rules/tariff.rule
```

Whether your rule fits a table at all is settled first on [Does your rule fit](fit.md).

### 2-2. Check it

```console
$ rulec check rules/shipping_fee.rule
```

A gap comes back **with the input that falls through it**, and the shape of the row to add. Only the amount is a person's to decide.

```console
error[E101]: Completeness gap: some input matches no row
  --> rules/tariff.rule:34 table 運賃表
   |
34 | table 運賃表(fee_table)
   |       ^^^^^^ the input space is not fully covered
   |
 An input that matches no row: あて先 = 山梨県, サイズ = S60
 hint: add a row that matches this input.
 The shape of the row to add: `| 山梨県 | S60 | 820円 |`. Its output values are copied from the first row to give a shape that parses; they are not the right amounts.
```

A contradiction stops with **an input both rows match**:

```console
error[E105]: Overlapping rows: the same input matches row 8 and row 22
  --> rules/tariff.rule:58 table 運賃表
 Both rows match: あて先 = 青森県, サイズ = S60
```

Fix until it passes. What you fix is the table, never code. The seven checks are on [What it proves](checks.md#the-seven).

### 2-3. Write the examples

The answers you decided go into `examples`, and every `check` runs them. The "for instance" of a spec becomes a test that does not go away.

```rule
examples
| 届け先 | 重量  | -> 送料 |
| 沖縄県 | 2500g | 1800円  |
| 東京都 | 1999g | 800円   |
```

### 2-4. Render the pages for approval and publication

From the same table, one page per reader. For the approver, the facts the table does not show: what was verified, which row hides which, which rounding is provisional. For the customer, no aliases and no diagnostic codes, and instead the answer on both sides of every threshold.

```console
$ rulec doc rules/shipping_fee.rule > shipping_fee.md
$ rulec doc rules/shipping_fee.rule --audience customer > shipping_fee_article.md
```

### 2-5. Know the impact of a revision before it ships

When a rule is revised, how many cases move and by how much can be known first. With past records (one JSON object per line), both versions are applied to the same records.

```console
$ rulec fixtures lint records.jsonl rules/shipping_fee.rule
records.jsonl: 70 records (70 observed, 0 filled)
No format problems.
$ rulec diff shipping_fee.rule shipping_fee_new.rule --fixtures records.jsonl --terse
Compared 70 / matched 60 (85.714%)
Counterpart: shipping_fee.rule → shipping_fee_new.rule

Affected 10 (14.286%)  amount +1,700
  table 基本送料 row 2 / table 負担判定 row 2                  3 records  difference +100 uniform  total +300
  table 基本送料 row 2 / table 負担判定 row 3                  7 records  difference +200 uniform  total +1,400
```

Where an implementation already runs, `rulec verify` holds the table to it before anything is replaced. Both are on [Compare and replay](compare.md).

---

## 3. Implementing from a new rule

You have a `.rule` that passes check and want it inside your app or your batch, in your language. The lead role here is the **generated code**; what you write is the caller.

![From a table that passed check, rulec gen writes code in nine languages and rulec test holds each to the reference evaluator. The implementer reads how to call it from rulec api and puts the function, the SQL query, the Wasm module or the MCP server into an app, a batch or an agent. Generated code is never edited; when the table changes, gen --check in CI stops the build](images/scenario-implementing.svg#only-dark)
![From a table that passed check, rulec gen writes code in nine languages and rulec test holds each to the reference evaluator. The implementer reads how to call it from rulec api and puts the function, the SQL query, the Wasm module or the MCP server into an app, a batch or an agent. Generated code is never edited; when the table changes, gen --check in CI stops the build](images/scenario-implementing-light.svg#only-light)

### 3-1. Generate

```console
$ rulec gen rules/shipping_fee.rule --out generated/
```

Under `generated/`, one directory per language. Python, TypeScript, JavaScript, Rust, Ruby, Go and Swift get a function; SQL gets one query over a relation of inputs; Wasm gets one module. No runtime, no dependency.

### 3-2. Read how to call it

You do not read the generated code to call it; the inventory says how.

```console
$ rulec api rules/shipping_fee.rule | jq -r .python.signature
def shipping_fee(dest: Prefecture, weight: Gram, total: YenInclTax, member: MemberKind) -> YenInclTax:
$ rulec api rules/shipping_fee.rule | jq -r .go.signature
func ShippingFee(in Input) (YenInclTax, error)
```

Values are integers in the declared unit (`1999` for 1,999 g, a rate as a number of steps) and enum members are spelled as the inventory spells them. The entry checks ranges and enums, so a value outside the declaration is refused rather than computed in silence. The details are on [Generate and call](generate.md#how-to-call-it-without-reading-it).

### 3-3. Hold every language to the table

Every generated language is run over the cases built from the table's boundaries and compared with the reference evaluator, byte for byte.

```console
$ rulec test generated/
ok    shipping_fee (Python) 68 vectors
ok    shipping_fee (TypeScript) 68 vectors
ok    shipping_fee (Go) 68 vectors
ok    shipping_fee (SQL) 68 vectors
ok    shipping_fee (Wasm) 68 vectors
…
```

A toolchain that is not installed is skipped, and the skip is reported.

### 3-4. Wire it in

Pick the shape the destination takes.

| Destination | What to use | Where to read |
|---|---|---|
| your application's code | the generated function | [Generate and call](generate.md#what-the-output-looks-like) |
| a recalculation in the database, a closing batch | the SQL query, one statement over the input relation | [SQL is a query, not a function](generate.md#sql-is-a-query-not-a-function) |
| a browser, or any host at all | the Wasm module | [Wasm](generate.md#wasm-one-module-for-any-host) |
| a place where an agent has to decide | the generated MCP server, the rule as one tool | [The rule as a tool for an agent](generate.md#the-rule-as-a-tool-for-an-agent) |
| a form or an API entry | the JSON Schema from `rulec schema` | [The input checked from the same table](generate.md#the-input-checked-from-the-same-table) |

Generated code is never edited. What you want changed is in the table, and a change to the table changes every language at once.

### 3-5. Keep it in step when the table changes

Commit the generated code and regenerate in CI with `--check`. A table that changed while its generated code did not stops the build there.

```console
$ rulec gen rules/ --out generated/ --check
```

The whole job is on the [install page](install.md#in-ci), with the two lines that post the impact of a revision to a pull request.

---

## Where next

[Write a table (.rule)](tour.md){ .md-button .md-button--primary }
[What it proves](checks.md){ .md-button }
[Generate and call](generate.md){ .md-button }
[Compare and replay](compare.md){ .md-button }
