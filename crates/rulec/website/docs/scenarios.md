# How to use it, by role

One tool, but **what you have in hand and what you want out** decide the path through it. Four readers, four paths. Start from the one closest to you.

| You are | What you have | What you want | Read |
|---|---|---|---|
| **implementing** a public rule (a statute, a published policy, a tariff) | the article, or the policy PDF | code that does what the article says, and that notices the amendment | [1. Implementing an existing public rule](#1-implementing-an-existing-public-rule) |
| **implementing a rule of your own that already exists**: an internal policy, your service's terms or tariff, a spreadsheet, and perhaps an implementation that runs today | the policy document, the spreadsheet, the running code | code that does what the document says and answers like the current implementation | [2. Implementing an existing rule of your own](#2-implementing-an-existing-rule-of-your-own) |
| **designing** a new rule (a new public rule, an internal rule, the terms of an online shop or a service) | the conditions, in prose or in your head | a table with no gap and no contradiction, and the pages that get it approved or published | [3. Designing a new rule](#3-designing-a-new-rule) |
| **implementing** from a finished rule | a `.rule` that passes check | code in your own language that answers exactly like the table | [4. Implementing from a new rule](#4-implementing-from-a-new-rule) |

The middle step is the same on every path: **nothing comes out of a table that does not pass `rulec check`** ([What it proves](checks.md)). Every command below is one of these:

```console
$ rulec --help
```

Every step below is one of three kinds: **something to hand to an agent, something rulec does, or something a person decides**. Transcribing an article or a policy into a table, drafting from a spreadsheet, the one line of an adapter around legacy code, wiring the generated code in: all of that an agent can do. Proving there is no gap and no overlap, pinning a source, holding the table to a legacy implementation or to past records, checking that twelve languages agree: rulec does that, mechanically. What stays with a person is deciding the conditions, approving, ruling on which side of a mismatch is wrong, and rereading a source after an amendment or a replacement. Under each heading below is who does that step.

---

## 1. Implementing an existing public rule

You have a statute or a published policy and want code that does exactly what it says. The lead role here is the **source**: which row came from which article, held to a copy of the text, so that an amendment is noticed.

![An agent transcribes a statute or a published policy into a table (.rule), citing the article with @. rulec holds the table to the copy, proves it has no gap and no overlap, and generates twelve languages. The approver compares the article and the table on the page rulec doc renders. When an amendment comes, rulec source outdated says so and the table is reread](images/scenario-existing.svg#only-dark)
![An agent transcribes a statute or a published policy into a table (.rule), citing the article with @. rulec holds the table to the copy, proves it has no gap and no overlap, and generates twelve languages. The approver compares the article and the table on the page rulec doc renders. When an amendment comes, rulec source outdated says so and the table is reread](images/scenario-existing-light.svg#only-light)

### 1-1. Transcribe, citing the article

Who: the agent

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

Who: the agent

`check` never reads the network. The article's copy lives beside the rule, and its digest is written into the rule. Two commands:

```console
$ rulec source fetch rules/stamp_duty.rule
措置法: fetched 第91条 (sha256:85faf53f6f6e8196)
$ rulec source pin rules/stamp_duty.rule
措置法: pinned 1 fragments
```

`pin` writes one line under the `source`: `  第91条 sha256:85faf53f6f6e8196`. The copies go to `sources/law/<law id>@<date>/`; commit them.

### 1-3. Check

Who: rulec. The agent fixes the table until it passes

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

Who: a person, the approver. rulec renders the page

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

Who: the agent. rulec checks that the twelve languages agree

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

Where an implementation already runs, hold the table to it before anything is replaced, as in [2-3](#2-3-hold-it-to-the-code-that-runs-today).

### 1-6. Notice the amendment

Who: rulec, weekly in CI. A person rereads the article and decides

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

## 2. Implementing an existing rule of your own

An internal policy, the terms or the tariff of your own service, a spreadsheet someone keeps, code that already runs. The rule is not public, but it is decided and in force, and you want code that does the same. The lead role here is the **comparison**: unlike a statute, the document cannot be fetched again, so it is pinned whole by its digest; and where an implementation or past records exist, the table is held to them and every mismatch comes back by row.

![An agent transcribes what is at hand - an internal policy, the terms of your own service, a spreadsheet, code that runs today - into a table (.rule): a spreadsheet becomes a draft through rulec import, a document's table is cited with @ and pinned by the digest of the copy. rulec proves no gap and no overlap, holds the table to the legacy implementation and to past records, and returns every mismatch by row, count and amount. The approver compares the document and the table on the page rulec doc renders. From a passed table come twelve languages](images/scenario-internal.svg#only-dark)
![An agent transcribes what is at hand - an internal policy, the terms of your own service, a spreadsheet, code that runs today - into a table (.rule): a spreadsheet becomes a draft through rulec import, a document's table is cited with @ and pinned by the digest of the copy. rulec proves no gap and no overlap, holds the table to the legacy implementation and to past records, and returns every mismatch by row, count and amount. The approver compares the document and the table on the page rulec doc renders. From a passed table come twelve languages](images/scenario-internal-light.svg#only-light)

### 2-1. Start from what you have

Who: the agent. A person confirms every line marked guess

A spreadsheet becomes a first draft. Every guess is marked, so only the marked places need a look.

```console
$ rulec import xlsx 運賃表.xlsx --sheet 本則 --name 運賃 > rules/運賃.rule
```

```rule
rule 運賃(imported) v1
description "A draft that rulec import made from 運賃表.xlsx (sheet 本則). Every line marked guess is for a person to confirm"

enum あて先_values(c1_kind) = 北海道(v1) | 沖縄県(v2) | 東京都(v3)  # guess: the values seen in this column, as an enum; add what is missing, and rename the aliases
enum 重量_values(c2_kind) = <=2000g(v1) | >2000g(v2)  # guess: the values seen in this column, as an enum; add what is missing, and rename the aliases

inputs
  あて先(c1) : あて先_values
  重量(c2) : 重量_values

outputs
  送料(o1) : money[円, incl_tax]  round down(1円)  # guess: the rounding's direction and grid come from the source; if it has none, write down that this is a placeholder; whether tax is included has to come from the source
…
```

A policy document is transcribed as in [1-1](#1-1-transcribe-citing-the-article). If all there is is the running code, hand that code to an agent to transcribe, and hold the table to the code in [2-3](#2-3-hold-it-to-the-code-that-runs-today). The running code is not touched.

### 2-2. Pin the document as a file

Who: the agent. rulec notices a replaced document

It cannot be fetched again, so the document itself sits beside the rule and its digest, whole, goes into the rule. It is cited as `@terms`, or — to say **which table of the document was transcribed** — as `@terms table1`, the first table of the document in document order (`表1` is the same name in Japanese).

```rule
source terms = file "shipping-terms.md"

table base_rate  @terms table1
policy unique
| dest               | weight | -> base : money[USD, incl_tax] |
| north_america      | <=10lb | 8USD                           |
| north_america      | >10lb  | 14USD                          |
| not: north_america | <=10lb | 26USD                          |
| not: north_america | >10lb  | 42USD                          |
```

Citing a table makes `rulec source fetch` take that table out of the document and write it beside it. A sheet is a table in a workbook (`.xlsx`), a `w:tbl` in a Word file (`.docx`), and what it looks like in Markdown and CSV. A PDF or a scan cannot be read here: hand an extractor (docling and the like) to `--via`, or cite the document whole as `@terms`.

```console
$ rulec source fetch rules/shipping_fee.rule
terms: took out table1 (3 rows by 3 columns, sha256:c846fef7727dd6e0)
$ rulec source pin rules/shipping_fee.rule
terms: pinned sha256:d1156fa90a72194c
terms: pinned 1 fragments
```

From here on, **an amount in the table has to be a value the copy shows**, which is what catches a mistyped digit.

When the document is replaced, `check` stops with E038 and names the tables that cite it; whether the file changed is known on the spot, from its digest. With a `url "…"` on it, `rulec source outdated` asks where it came from, and says whether a cited table moved or only something this rule does not transcribe.

```console
$ rulec check rules/shipping_fee.rule
error[E038]: The copy of source `terms` has changed
  --> rules/shipping_fee.rule:6 source terms
  |
6 | source terms = file "shipping-terms.md" sha256:a4b42e3e6c346e56
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ pinned: sha256:a4b42e3e6c346e56
  |
 The copy now: sha256:a4b1a4052a111009
 Definitions to reread: table base_rate, table surcharge
 Reread the document; if what was transcribed still holds, rewrite the line as follows to pin the new copy.
```

A mistyped digit looks like this. `14USD` written as `11USD` sits on the rounding grid, leaves no gap and overlaps nothing: every other check stays green and these two are what fail.

```console
$ rulec check rules/shipping_fee.rule
error[E116]: The amount of row 4 is not in the copy it cites
  --> rules/shipping_fee.rule:24 table base_rate row 2
   |
24 | | north_america | >10lb | 11USD |
   |                           ^^^^^ not in the copy: 11USD
   |
 The copy cited: terms table1
 An amount is not rewritten as it is transcribed, so either it was mistyped or it came from somewhere else. …

warning[W120]: The copy of table1 states values no row uses
 Stated in the copy, used by no row: 14USD
```

The two name both halves of the same slip. W120 also catches **a row that was never transcribed** — the completeness check cannot, because the inputs of a dropped row fall into one of the rows that remain.

The approver's page quotes the copy under the table's heading — "Source: terms table1 (shipping-terms.md, sha256:d1156fa90a72194c)", then the table itself — and adds one line to what was verified: *Every amount in this table is a value the copy it cites (terms table1) shows (E116)*. The source table above, the rule's table below, and that line between them.

### 2-3. Hold it to the code that runs today

Who: the agent, for the adapter's one line and verify. A person rules on each mismatch

Where an implementation already runs, hold the table to it before anything is replaced. The legacy code is wrapped in an adapter of about twenty lines, and the cases built from the table's boundaries go through both. rulec prints the adapter's template; the one line to write is the call into the legacy code. The legacy code itself is not touched.

```console
$ rulec adapter rules/shipping_fee.rule --template python > adapter.py
```

```python
# rulec adapter template (rule 送料).
# It only exchanges JSON Lines over stdin/stdout. Call the legacy implementation from here.
import json, sys

sys.stdin.readline()  # handshake
print(json.dumps({"ok": True, "impl": "legacy@REPLACE_ME"}), flush=True)

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    d = req["in"]  # inputs: 届け先, 重量, 注文金額, 会員

    # Call the legacy implementation here.
    got = 0  # TODO: legacy.compute(d)

    print(json.dumps({"id": req["id"], "out": {"送料": got}}, ensure_ascii=False), flush=True)
```

```console
$ rulec verify rules/shipping_fee.rule --adapter python3 adapter.py
Compared 70 / matched 60 (85.714%)
Counterpart: legacy@2024-03

Affected 10 (14.286%)  amount +2,550
  table 基本送料 row 2 / table 負担判定 row 2                  3 records  difference +150 uniform  total +450
    Example: 会員=プラチナ, 届け先=北海道, 注文金額=0, 重量=2001 → rule 送料=900 / legacy 送料=750
  table 基本送料 row 2 / table 負担判定 row 3                  7 records  difference +300 uniform  total +2,100
    Example: 会員=一般, 届け先=北海道, 注文金額=0, 重量=2001 → rule 送料=1800 / legacy 送料=1500
```

Mismatches come grouped by the rows that matched. Here only the row for remote areas above 2 kg disagrees. Whether that is a defect in the legacy code, a transcription error in the table or a rounding convention is decided from the row and its example; a mismatch is not automatically anyone's bug.

### 2-4. Hold it to past records

Who: the agent. A person rules on each mismatch

Without running code, but with records of past cases (the inputs and the values that came out), the rule is applied to the records. Records are one JSON object per line; their shape is checked first, then they are replayed.

```console
$ rulec fixtures lint records.jsonl rules/shipping_fee.rule
records.jsonl: 70 records (70 observed, 0 filled)
No format problems.
$ rulec replay rules/shipping_fee.rule --fixtures records.jsonl --terse
Compared 70 / matched 60 (85.714%)
Counterpart: records.jsonl

Affected 10 (14.286%)  amount -1,700
  table 基本送料 row 2 / table 負担判定 row 2                  3 records  difference -100 uniform  total -300
  table 基本送料 row 2 / table 負担判定 row 3                  7 records  difference -200 uniform  total -1,400
```

Both comparisons are described on [Compare and replay](compare.md).

### 2-5. Approve and generate

Who: a person approves, the agent generates

The approver gets the page `rulec doc` renders, with the document quoted under each table's heading, as in [1-4](#1-4-show-it-to-the-approver). Generating and holding the twelve languages to the table is [4. Implementing from a new rule](#4-implementing-from-a-new-rule).

---

## 3. Designing a new rule

Shipping fees, coupon conditions, whether a return is accepted, an internal criterion, a new public rule: something still being decided that you want to settle as a table. The lead role here is the **check**: every gap and every contradiction comes back with a concrete input that shows it, so what you forgot to decide is visible before you decide.

![Someone designing a rule (shipping, coupons, returns, an internal criterion) writes it as a table (.rule). rulec check returns every gap and overlap with an input that shows it, until the table passes. From a passed table come the approver's page, the customer article and the impact of a revision](images/scenario-designing.svg#only-dark)
![Someone designing a rule (shipping, coupons, returns, an internal criterion) writes it as a table (.rule). rulec check returns every gap and overlap with an input that shows it, until the table passes. From a passed table come the approver's page, the customer article and the impact of a revision](images/scenario-designing-light.svg#only-light)

### 3-1. Write the table first

Who: a person decides the conditions. The agent or the person writes the table

Conditions as columns, the answer as the last column. "Canada and the US, eight dollars up to ten pounds" becomes one row.

```rule
rule shipping_fee v1
description "Standard delivery fee"

enum zone = domestic | canada | overseas
group north_america = domestic, canada

inputs
  dest   : zone
  weight : mass[lb]  range >=1lb <=70lb

outputs
  fee : money[USD, incl_tax]  round up(1USD)

table base
policy unique
| dest               | weight | -> fee : money[USD, incl_tax] |
| north_america      | <=10lb | 8USD                          |
| north_america      | >10lb  | 14USD                         |
| not: north_america | <=10lb | 26USD                         |
| not: north_america | >10lb  | 42USD                         |
```

If the rule already lives in a spreadsheet, a first draft can be made from it. Every guess is marked, so only the marked places need a look.

```console
$ rulec import csv tariff.csv --name 運賃 > rules/tariff.rule
```

Whether your rule fits a table at all is settled first on [Does your rule fit](fit.md).

### 3-2. Check it

Who: rulec. A person answers each witness

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

### 3-3. Write the examples

Who: the person who decided the answers. The agent may write them into the file

The answers you decided go into `examples`, and every `check` runs them. The "for instance" of a spec becomes a test that does not go away.

```rule
examples
| dest     | weight | -> fee |
| overseas | 12lb   | 42USD  |
| domestic | 9lb    | 8USD   |
```

### 3-4. Render the pages for approval and publication

Who: rulec (rulec doc). The approver and the customer read

From the same table, one page per reader. For the approver, the facts the table does not show: what was verified, which row hides which, which rounding is provisional. For the customer, no aliases and no diagnostic codes, and instead the answer on both sides of every threshold.

```console
$ rulec doc rules/shipping_fee.rule > shipping_fee.md
$ rulec doc rules/shipping_fee.rule --audience customer > shipping_fee_article.md
```

### 3-5. Know the impact of a revision before it ships

Who: rulec (rulec diff). A person decides whether it ships

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

Where an implementation already runs, [2-3](#2-3-hold-it-to-the-code-that-runs-today) holds the table to it before anything is replaced. Both are on [Compare and replay](compare.md).

---

## 4. Implementing from a new rule

You have a `.rule` that passes check and want it inside your app or your batch, in your language. The lead role here is the **generated code**; what you write is the caller.

![From a table that passed check, rulec gen writes code in twelve languages and rulec test holds each to the reference evaluator. The implementer reads how to call it from rulec api and puts the function, the SQL query, the Wasm module or the MCP server into an app, a batch or an agent. Generated code is never edited; when the table changes, gen --check in CI stops the build](images/scenario-implementing.svg#only-dark)
![From a table that passed check, rulec gen writes code in twelve languages and rulec test holds each to the reference evaluator. The implementer reads how to call it from rulec api and puts the function, the SQL query, the Wasm module or the MCP server into an app, a batch or an agent. Generated code is never edited; when the table changes, gen --check in CI stops the build](images/scenario-implementing-light.svg#only-light)

### 4-1. Generate

Who: the agent

```console
$ rulec gen rules/shipping_fee.rule --out generated/
```

Under `generated/`, one directory per language. Python, TypeScript, JavaScript, Rust, Ruby, PHP, Go, Swift and Java get a function; SQL gets one query over a relation of inputs and the same query as a PostgreSQL function; Wasm gets one module; NumPy gets the rule as data and one fixed evaluator. No runtime, no dependency.

### 4-2. Read how to call it

Who: the agent

You do not read the generated code to call it; the inventory says how.

```console
$ rulec api rules/shipping_fee.rule | jq -r .python.signature
def shipping_fee(dest: Prefecture, weight: Gram, total: YenInclTax, member: MemberKind) -> YenInclTax:
$ rulec api rules/shipping_fee.rule | jq -r .go.signature
func ShippingFee(in Input) (YenInclTax, error)
```

Values are integers in the declared unit (`1999` for 1,999 g, a rate as a number of steps) and enum members are spelled as the inventory spells them. The entry checks ranges and enums, so a value outside the declaration is refused rather than computed in silence. The details are on [Generate and call](generate.md#how-to-call-it-without-reading-it).

### 4-3. Hold every language to the table

Who: rulec (rulec test)

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

### 4-4. Wire it in

Who: the agent

Pick the shape the destination takes.

| Destination | What to use | Where to read |
|---|---|---|
| your application's code | the generated function | [Generate and call](generate.md#what-the-output-looks-like) |
| a recalculation in the database, a closing batch | the SQL query, one statement over the input relation | [SQL is a query, and the same query as a function](generate.md#sql-is-a-query-and-the-same-query-as-a-function) |
| an HTTP endpoint with no server of your own | the same query as a PostgreSQL function, behind PostgREST or Supabase | [SQL is a query, and the same query as a function](generate.md#sql-is-a-query-and-the-same-query-as-a-function) |
| a browser, or any host at all | the Wasm module | [Wasm](generate.md#wasm-one-module-for-any-host) |
| a place where an agent has to decide | the generated MCP server, the rule as one tool | [The rule as a tool for an agent](generate.md#the-rule-as-a-tool-for-an-agent) |
| a form or an API entry | the JSON Schema from `rulec schema` | [The input checked from the same table](generate.md#the-input-checked-from-the-same-table) |

Generated code is never edited. What you want changed is in the table, and a change to the table changes every language at once.

### 4-5. Keep it in step when the table changes

Who: CI (rulec gen --check)

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
