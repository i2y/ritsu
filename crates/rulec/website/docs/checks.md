# What it proves

`rulec check` is the centre of the tool. Everything else — the code
generation, the vectors, the replay — is downstream of it, and a rule
that does not pass it does not generate.

```console
$ rulec check rules/
note rules/ゆうパック運賃.rule: 21 shadow pairs (21 structural, 0 equivalent, 0 needs review)
ok rules/ゆうパック運賃.rule
```

Exit codes are **0** (notes only), **1** (errors), **2** (bad arguments
or an unreadable file). Read the exit code, not whether the output looks
empty.

## The seven

| | |
|---|---|
| **Completeness** | if some input matches no row, it stops — **with that input** |
| **Overlap** | under `policy unique`, an overlap is an error. Under `policy first`, structural shadowing (the staircase) is told apart from the pairs whose outputs differ and therefore deserve a decision |
| **Dead rows** | a row nothing reaches. The message tells apart "earlier rows already cover it" from "the upstream table never emits the value it names" |
| **Units** | adding yen to grams stops. So does tax-inclusive plus tax-exclusive |
| **Rounding** | a numeric output must declare one. Without it, the message shows the money: "down(1円) gives 701, half_up(1円) gives 702, up(10円) gives 710 — the mode moves it by up to 9 yen" |
| **Overflow** | that every intermediate fits in int64, proved from the declared ranges and steps |
| **Examples** | every example runs; a failure names the rows that fired; a missing output column stops |

Every diagnostic writes its first line in the words of the business,
**always carries a concrete case**, and states the fix down to the
rewritten form.

## Reading a diagnostic

```
error[E101]: Completeness gap: some input matches no row
  --> rules/ゆうパック運賃.rule:34 table 運賃表
   |
34 | table 運賃表(fee_table)
   |       ^^^^^^ the input space is not fully covered
   |
 An input that matches no row: あて先 = 山梨県, サイズ = S60
 hint: add a row that matches this input.
 The shape of the row to add: `| 山梨県 | S60 | 820円 |`. Its output values are copied
 from the first row to give a shape that parses; they are not the right amounts. …
```

The witness is the part to reason about. `あて先 = 山梨県, サイズ = S60`
is not an illustration — it is an input the checker constructed, and it
is the sentence you hand to whoever knows the answer.

### `--terse`, when there are many

```console
$ rulec check rules/ --terse
error[E101]: Completeness gap: some input matches no row
  --> rules/ゆうパック運賃.rule:34 table 運賃表
  witness: あて先 = 山梨県, サイズ = S60
…
details: rulec explain <code>
```

Heading, position, witness. The pointer to the rest is printed once, at
the end of the run.

### `--format json`, when a program is reading

The same finding as data. The prose is still there, but nothing
downstream has to take a sentence apart:

```console
$ rulec check rules/ゆうパック運賃.rule --format json | jq -c 'select(.code=="E101") | {table:.where.table, witness:.witness.inputs, fix:.fix}'
{"table":"運賃表","witness":{"あて先":"山梨県","サイズ":"S60"},"fix":{"kind":"add_row","text":"| 山梨県 | S60 | 820円 |"}}
```

`where` says which table and row, `witness` is an assignment of values
in the canonical unit, `rows` names every row that takes part, and `fix`
carries the rewritten form ready to paste. **`witness` and `fix` do not
change with `--lang`** — only fields whose names say prose do. The full
shape is in [Formats](formats.md#check).

!!! warning "`fix.text` is a form, not a decision"

    It parses, and it removes the code — both are tested. It does not
    know the right amount, the right rounding direction or the right
    grid. Those are business decisions, and the caveat sits in `notes`,
    which is prose, because `fix.text` is bytes.

## Looking a code up

Every code has an entry: when it appears, how to fix it down to the
rewritten form, the **smallest `.rule` that reproduces it**, and the
codes next to it.

```console
$ rulec explain E101
error[E101]: Completeness gap: some input matches no row

When
  The union of the rows does not cover the declared input space. …

Fix
  Add a row that matches the witness. If a new enum value caused it, …

Smallest reproduction
  rule t(t) v1

  enum k(k) = a(a) | b(b) | c(c)
  …

Related codes: E102 E105 W111
```

`rulec explain --all --format markdown` is the whole ledger — and is
exactly what [Diagnostics](codes.md) on this site is built from. Every
reproduction in it is run by the test suite, so an example cannot rot
into something that reads well and is no longer true.

## Only what is new

```console
$ rulec check rules/ --diff-base origin/main
```

Findings that were already present at that revision are hidden, and the
count of what was hidden is printed. Pairs are matched on the canonical
form of their cells rather than on line numbers, so inserting one row
does not make everything look new.

That is what makes warnings usable in CI: a table can carry an intended
`policy first` shadowing for years without drowning the one overlap this
change introduced.

## When it cannot prove something

It says so. It does not approximate and pass.

- **E109** — the check ran out of its node budget, so completeness was
  not proved. Split the table or raise `--budget`; nothing is assumed.
- **W114** — two rows of a `unique` table might overlap, and the checker
  could neither construct an input that proves it nor prove that none
  exists. It warns, and **the generated code carries a guard** that
  returns an error rather than silently picking the earlier row. If that
  guard ever fires, the overlap was real.

## Showing it to the person who approves

```console
$ rulec doc rules/ゆうパック運賃.rule --lang ja > 運賃.md
```

A `.rule` is already almost markdown, so transcribing its syntax is
worth nothing. What `doc` adds is **the facts the checker knows that the
text does not show**: that a group of six values and its complement of
41 really do cover all 47, which rows shadow which, where a rounding was
assumed rather than sourced.

This is what `rulec doc` writes (excerpt; the command above asked for Japanese).

```markdown
## グループ

グループは列挙の一部に名前を付けたものです。表のセルに書かれた一語が、下の値をまとめて指しています。

- **近畿圏**（6 値）— 滋賀県、京都府、大阪府、兵庫県、奈良県、和歌山県
- **中国四国**（9 値）— 鳥取県、島根県、岡山県、広島県、山口県、徳島県、香川県、愛媛県、高知県
- **沖縄**（1 値）— 沖縄県
…

この 6 グループは 都道府県 の 47 値を過不足なく分割しています（この資料が宣言から数えました）。

## 表 運賃表（policy unique）

| 列 | 出どころ |
|---|---|
| あて先 | 入力 |
| サイズ | 表 サイズ判定 の出力 |
| → 運賃 | この規則の出力 |
…
```

There is one prohibition. **It writes no sentence that is not in the
checker's output** — every line traces back to the source or to a check
result, and the two are named apart ("rulec check confirmed" versus
"this rendering counted it from the declarations"). It is never
committed: CI renders it and pastes it into the PR, because a stale
rendering that still looks authoritative is the danger it was designed
against.

---

[The diagnostics ledger](codes.md){ .md-button .md-button--primary }
[Generate and call](generate.md){ .md-button }
