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
$ rulec adapter rules/yupack_base_fee.rule --template python > adapter.py
$ rulec schema rules/yupack_base_fee.rule            # the JSON Schema of the wire
$ rulec verify rules/yupack_base_fee.rule --adapter python3 adapter.py
Compared 209 / matched 184 (88.038%)
Counterpart: legacy@fake-1

Affected 25 (11.962%)  amount -250
  table size_of row 1 / table fee_table row 36         7 records  difference -10 uniform  total -70
    Example: dest=沖縄県, girth=1, weight=1 → rule fee=1450 / legacy fee=1460
  …
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
$ rulec fixtures lint replay/2025-08.jsonl rules/yupack_base_fee.rule
replay/2025-08.jsonl: 209 records (209 observed, 0 filled)

1 problems:
  `in.dest`: `江戸` is not a value of enum 都道府県
    1 record(s). Example: line 21 (order:b3)
    The type or range disagrees with the declaration.
```

Broken records are **reported, not discarded**. Dropping them silently
would raise the match rate by exactly however much the denominator
shrank.

```console
$ rulec replay rules/yupack_base_fee.rule --fixtures replay/2025-08.jsonl
```

Records written by the generated code carry the rows that matched (the `_record`
function writes them), and `replay` compares those too: a record whose amount agrees but
whose row differs from the rule's is reported apart, as a moved row, clustered by the move.

## Between two versions with no records at all

Records tell you how many of *your* cases move. They cannot tell you
about a case you have never seen. Leave `--fixtures` off and the same
command answers that instead:

```console
$ rulec diff member_shipping_fee@v3 member_shipping_fee@v4
rule member_shipping_fee v3 → v4
7050 cells, of which 3525 are inputs that can occur: 3501 same, 24 differ, 0 unsettled, 0 unrealized

  member not platinum  and  weight >=2001g <=40000g  and  total >=0JPY <=29999JPY  and  dest = remote
    fee: 1800 → 2000
    rows: table base row 2, table payer row 3
    example: dest=北海道, member=basic, total=0, weight=2001

  member = platinum  and  weight >=2001g <=40000g  and  total >=0JPY <=29999JPY  and  dest = remote
    fee: 900 → 1000
    rows: table base row 2, table payer row 2
    example: dest=北海道, member=platinum, total=0, weight=2001

outside this region the two versions answer alike.
```

Two things in that answer are not available from a log.

**`total >=0JPY <=29999JPY`.** The change was one amount in the base fee
table, and nothing about it mentions the order total. But an order of
30,000 yen or more pays 0% of the base fee, and zero times the new
amount is zero times the old one: the change is *erased* on that side.
The region is what the whole rule does with the change, not what the
changed row says.

**The last line.** Outside the region, the two versions are the same —
not "the records we had did not show a difference", but the same. That
is a claim, and it is withheld when it was not earned: a rule that folds
a sequence is not a function of finitely many columns, and two derived
columns that share an input can ask for a combination the sieve cannot
rule out and no input can be built for ([the blind spot W114 already
names](checks.md)). Those parts are reported as parts that could not be
settled, with the region they cover, rather than passed over.

How it works: both versions' boundaries are put on one set of axes —
**the rule's columns**, not its inputs. A derived column can cut the
input space diagonally (`spare = floor - used` tested at `<10m2` is a
slab, not a box), so a region over inputs alone could not be written
down, and a tool that tried would answer "no difference" where there is
one. Each cell of the refinement is settled three ways: the same
computation ran, so they agree over the whole cell; an input was found
where they disagree; or neither, which says so. The differing cells are
covered with boxes again and written in the notation a cell is written
in.

The two answers are meant to be held against each other. Run this
first — it says what *can* change — and `--fixtures` second, which says
how many of your records land in it. They agree over the whole corpus,
and `tests/vdiff.rs` is what holds them to it.

## Between two versions, over the records you have

```console
$ rulec diff yupack_base_fee@v1 yupack_base_fee@v2 --fixtures replay/2025-08.jsonl
Compared 209 / matched 200 (95.694%)
Counterpart: yupack_base_fee@v1 → yupack_base_fee@v2

Affected 9 (4.306%)  amount +2,800
  table size_of row 1→row 2 / table fee_table row 15→row 16     1 record  difference +320 uniform  total +320
    Example: dest=滋賀県, girth=60, weight=1 → rule fee=1310 / old version fee=990
```

`yupack_base_fee@v2` is sugar for the git tag `rules/yupack_base_fee/v2`, or,
when there is no such tag, for `v2` as a git revision. A path at a
revision — `rules/yupack_base_fee.rule@origin/main` — is the file as it is on
that branch, which is what a pull request compares against.
A diff clusters on **the transition of the fired row** — `row 1→row 2`
says where the decision moved — and each cluster carries the count, the
total amount, the minimum and maximum, and a witness. A uniform shift
folds into one line.

`--format markdown` produces what goes into a PR, and `--terse` leaves the
witness column out of it, because a comment is read by everyone with
access to the repository and the values of a production record are not
for it. Posting is one line of CI, so that the tool owns the formatting
and nothing else. `diff` exits 1 when there is an impact, which is the
information here and not a failure, so the step goes on after 1:

```yaml
- run: rulec diff rules/shipping_fee.rule@origin/main rules/shipping_fee.rule --fixtures "$FIXTURES" --format markdown --terse > diff.md || [ $? -eq 1 ]
  env:
    RULEC_LANG: ja        # the people approving this one read Japanese
- run: gh pr comment "$PR" --body-file diff.md
```

The whole job, checkout and install included, is on the
[install page](install.md#in-ci).

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
Filled records: 4 (weight: 4); matched 4. Not included in the headline match rate
Default values used: weight = 1000
```

The defaults are not written into the `.rule`, and that is deliberate: a
rule is a pure function, and filling is a judgement about one particular
replay experiment. Running "fill member with basic" and "fill it with gold
to see the upper bound of the impact" against the same rule is a
legitimate thing to do, and burning one of them into the rule would make
it impossible.

!!! warning "Fixtures do not go in the repository"

    They contain order amounts. Pass them to CI as an artifact or from
    protected storage. The manifest holds only field names and default
    values, so that one can be committed.

## Everything here is machine-readable too

```console
$ rulec verify rules/yupack_base_fee.rule --format json --adapter python3 adapter.py
{"compared":209,"matched":184,"rate":0.88038,"counterpart":"legacy@fake-1","unanswered":0,
 "clusters":[{"rows":[{"table":"size_of","row":1},{"table":"fee_table","row":36}],"count":7,
              "delta":{"fee":{"min":-10,"max":-10,"uniform":true,"total":-70}},
              "witness":{"in":{"dest":"沖縄県","girth":1,"weight":1},"ours":{"fee":1450},"theirs":{"fee":1460}},
              "records":[{"line":42,"tag":""},{"line":115,"tag":""},…],
              "suspect_rounding":false},…],
 "moved":[],"excluded":{},"filled":{"count":0,"by_field":{},"defaults":{}},"cases":null}
```

The same shape for all three commands, defined in
[Formats](formats.md#verify-replay-diff).

---

[For agents](agents.md){ .md-button .md-button--primary }
[Formats](formats.md){ .md-button }
