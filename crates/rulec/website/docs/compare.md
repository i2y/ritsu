# Compare and replay

This is the part that turns **"deploy it and watch the numbers"** into
**"look before deploying"**.

It comes in three, and they are the same machinery pointed at different
counterparts: an implementation that exists today (`verify`), what
actually happened (`replay`), and the other version of the rule
(`diff`). All three cluster their mismatches by the rows that fired and
report counts, amount differences and a witness.

## Against a legacy implementation

Write a 20-to-30-line adapter — rulec prints the shape:

```console
$ rulec adapter rules/送料.rule --template python > adapter.py
$ rulec schema rules/送料.rule                      # the JSON Schema of the wire
$ rulec verify rules/ゆうパック運賃.rule --adapter python3 adapter.py
Compared 207 / matched 182 (87.923%)
Counterpart: legacy@fake-1

Affected 25 (12.077%)  amount -250
  table サイズ判定 row 1 / table 運賃表 row 36                 7 records  difference -10 uniform  total -70
    Example: あて先=沖縄県, 三辺合計=1, 重量=1 → rule 運賃=1450 / legacy 運賃=1460
```

The legacy implementation is **started as a child process and spoken to
in JSON Lines over stdin and stdout**, so its language and its location
do not matter. Mismatches are clustered by the rows that fired, with
counts, amounts and a witness, and a cluster whose differences are all
smaller than the output's rounding grid is tagged "suspected rounding
difference".

A record the adapter says it cannot answer is **excluded from the
denominator** and reported separately — otherwise refusing the hard
cases would raise the match rate.

!!! note "A mismatch is not automatically your bug"

    It is one of four things: a defect in the legacy implementation, a
    transcription error in the table, a dirty record, or a rounding
    convention. The cluster and its witness are what tell them apart —
    and whether that triage really works is the one thing about this
    tool that cannot be settled without real data.

## Against what actually happened

Past records are JSON Lines, one per record. Validate them first:

```console
$ rulec fixtures lint replay/2025-08.jsonl rules/ゆうパック運賃.rule
replay/2025-08.jsonl: 208 records (208 observed, 0 filled)

5 problems:
  `in.あて先`: `江戸` is not a value of enum 都道府県
    1 record(s). Example: line 21 (order:b3)
    The type or range disagrees with the declaration.
```

Broken records are **reported, not discarded**. Dropping them silently
would raise the match rate by exactly however much the denominator
shrank.

```console
$ rulec replay rules/ゆうパック運賃.rule --fixtures replay/2025-08.jsonl
```

## Between two versions

```console
$ rulec diff ゆうパック運賃@v1 ゆうパック運賃@v2 --fixtures replay/2025-08.jsonl
Compared 207 / matched 190 (91.787%)
Counterpart: ゆうパック運賃@v1 → ゆうパック運賃@v2

Affected 17 (8.213%)  amount +5,300
  table サイズ判定 row 1→row 2 / table 運賃表 row 29→row 30    7 records  difference +300 uniform  total +2,100
    Example: あて先=北海道, 三辺合計=60, 重量=1 → rule 運賃=1710 / old version 運賃=1410
```

`ゆうパック運賃@v2` is sugar for the git tag `rules/ゆうパック運賃/v2`.
A diff clusters on **the transition of the fired row** — `row 1→row 2`
says where the decision moved — and each cluster carries the count, the
total amount, the minimum and maximum, and a witness. A uniform shift
folds into one line.

`--format markdown` produces what goes into a PR. Posting is one line of
CI, so that the tool owns the formatting and nothing else:

```yaml
- run: rulec diff 送料@v3 送料@v4 --fixtures "$FIXTURES" --format markdown > diff.md
  env:
    RULEC_LANG: ja        # the people approving this one read Japanese
- run: gh pr comment "$PR" --body-file diff.md
```

## Filling in a missing field is always stamped

When a record is missing a field, rulec does exactly two things: drop
the record entirely, or fill it from a **default declared in a replay
manifest** and mark it a *filled* record. It never infers the value
backwards.

The headline match rate is computed **from the observed records alone**,
and the report itself always writes down how many were filled and with
what:

```
Excluded 5 records (not matching the declared format)
Filled records: 4 (重量: 4); matched 4. Not included in the headline match rate
Default values used: 重量 = 1000
```

The defaults are not written into the `.rule`, and that is deliberate: a
rule is a pure function, and filling is a judgement about one particular
replay experiment. Running "fill 会員 with 一般" and "fill it with ゴールド
to see the upper bound of the impact" against the same rule is a
legitimate thing to do, and burning one of them into the rule would make
it impossible.

!!! warning "Fixtures do not go in the repository"

    They contain order amounts. Pass them to CI as an artifact or from
    protected storage. The manifest holds only field names and default
    values, so that one can be committed.

## Everything here is machine-readable too

```console
$ rulec verify rules/送料.rule --format json --adapter python3 adapter.py
{"compared":207,"matched":182,"rate":0.87923,"counterpart":"legacy@fake-1","unanswered":0,
 "clusters":[{"rows":[{"table":"サイズ判定","row":1},{"table":"運賃表","row":36}],"count":7,
              "delta":{"運賃":{"min":-10,"max":-10,"uniform":true,"total":-70}},
              "witness":{"in":{…},"ours":{"運賃":1450},"theirs":{"運賃":1460}},
              "suspect_rounding":false}],
 "excluded":{},"filled":{"count":0,"by_field":{},"defaults":{}}}
```

The same shape for all three commands, defined in
[Formats](formats.md#verify-replay-diff).

---

[For agents](agents.md){ .md-button .md-button--primary }
[Formats](formats.md){ .md-button }
