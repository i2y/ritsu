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
$ rulec adapter rules/parcel_rate.rule --template python > adapter.py
$ rulec schema rules/parcel_rate.rule            # the JSON Schema of the wire
$ rulec verify rules/parcel_rate.rule --adapter python3 adapter.py
Compared 96 / matched 79 (82.292%)
Counterpart: legacy@fake-1

Affected 17 (17.708%)  amount -17
  table size_of row 1 / table base_rate row 6 / table fuel_rate row 2 / table signature_fee row 1    10 records  difference -1 uniform  total -10
    Example: dest=overseas, girth=1, signature=true, weight=1 → rule fee=22 / legacy fee=23
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
$ rulec fixtures lint replay/2025-08.jsonl rules/parcel_rate.rule
replay/2025-08.jsonl: 95 records (95 observed, 0 filled)

1 problems:
  `in.dest`: `mars` is not a value of enum zone
    1 record(s). Example: line 21 (order:21)
    The type or range disagrees with the declaration.
```

Broken records are **reported, not discarded**. Dropping them silently
would raise the match rate by exactly however much the denominator
shrank.

```console
$ rulec replay rules/parcel_rate.rule --fixtures replay/2025-08.jsonl
```

Records written by the generated code carry the rows that matched (the `_record`
function writes them), and `replay` compares those too: a record whose amount agrees but
whose row differs from the rule's is reported apart, as a moved row, clustered by the move.

## Between two versions with no records at all

Records tell you how many of *your* cases move. They cannot tell you
about a case you have never seen. Leave `--fixtures` off and the same
command answers that instead:

```console
$ rulec diff parcel_rate@v3 parcel_rate@v4
rule parcel_rate v3 → v4
210 cells, of which 210 are inputs that can occur: 194 same, 16 differ, 0 unsettled, 0 unrealized

  signature = false  and  dest = north_america  and  weight >=11lb <=70lb  and  girth >=23in <=60in
    fee: 19 → 20
    rows: table size_of row 2, table base_rate row 3, table fuel_rate row 1, table signature_fee row 2
    example: dest=domestic, girth=23, signature=false, weight=11

  signature = true  and  dest = north_america  and  weight >=11lb <=70lb  and  girth >=23in <=60in
    fee: 23 → 24
    rows: table size_of row 2, table base_rate row 3, table fuel_rate row 1, table signature_fee row 1
    example: dest=domestic, girth=23, signature=true, weight=11

outside this region the two versions answer alike.
```

Two things in that answer are not available from a log.

**`weight >=11lb <=70lb  and  girth >=23in <=60in`.** The change was one
rate in the fuel table, 5% to 6% for north_america, and it reaches every
parcel that goes there. But the fee is rounded up to the dollar, and on
every other row the new surcharge comes to the same dollar as the old
one: a 12USD base is 12.60USD at 5% and 12.72USD at 6%, and both are
13USD. Only a small parcel over 160oz, on an 18USD base, crosses a
dollar (18.90USD to 19.08USD): the change is *erased* everywhere else.
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
$ rulec diff parcel_rate@v1 parcel_rate@v2 --fixtures replay/2025-08.jsonl
Compared 95 / matched 86 (90.526%)
Counterpart: parcel_rate@v1 → parcel_rate@v2
Excluded 1 record (not matching the old version's declared format)

Affected 9 (9.474%)  amount -91
  table size_of row 2→row 1 / table base_rate row 2→row 1 / table fuel_rate row 1 / table signature_fee row 1     4 records  difference -6 uniform  total -24
    Example: dest=domestic, girth=23, signature=true, weight=1 → rule fee=11 / old version fee=17
  …
```

Here v2 lets an envelope run to 24in instead of 22in.
`parcel_rate@v2` is sugar for the git tag `rules/parcel_rate/v2`, or,
when there is no such tag, for `v2` as a git revision. A path at a
revision — `rules/parcel_rate.rule@origin/main` — is the file as it is on
that branch, which is what a pull request compares against.
A diff clusters on **the transition of the fired row** — `row 2→row 1`
says where the decision moved — and each cluster carries the count, the
total amount, the minimum and maximum, and a witness. A uniform shift
folds into one line.

`diff` reads the records the way the old version reads them, because the
old version's system wrote them, and it reads only their inputs: what
came out at the time plays no part in comparing two versions. So a
record whose answer the change moves past anything the new version
could have answered is compared like any other. The one record excluded
above has a `dest` that neither version has; `fixtures lint` against the
old version names it.

An input the new version does not take is not excluded either: a range
it narrows, a value its enum no longer has, a `constraint` it adds, a
step a recorded value is no whole number of. The record counts as
compared and not matched, and the report lists it apart, by reason. Here
a pull request caps the weight at 50lb:

```console
$ rulec diff rules/parcel_rate.rule@origin/main rules/parcel_rate.rule --fixtures replay/2025-08.jsonl
Compared 95 / matched 73 (76.842%)
Counterpart: rules/parcel_rate.rule@origin/main → rules/parcel_rate.rule
Excluded 1 record (not matching the old version's declared format)

Not accepted by the new version 22 (23.158%)
  weight is outside the new version's range 1lb..50lb    22 records
    Example: dest=domestic, girth=2, signature=true, weight=69 → old version fee=11
```

The JSON names every one of them under `refused`, with the `kind` of
the reason.

`--format markdown` produces what goes into a PR, and `--terse` leaves the
witness column out of it, because a comment is read by everyone with
access to the repository and the values of a production record are not
for it. Posting is one line of CI, so that the tool owns the formatting
and nothing else. `diff` exits 1 when there is an impact, which is the
information here and not a failure, so the step goes on after 1:

```yaml
- run: rulec diff rules/shipping_fee.rule@origin/main rules/shipping_fee.rule --fixtures "$FIXTURES" --format markdown --terse > diff.md || [ $? -eq 1 ]
  env:
    RULEC_LANG: ja        # the people who read this one read Japanese
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
$ rulec verify rules/parcel_rate.rule --format json --adapter python3 adapter.py
{"compared":96,"matched":79,"rate":0.82292,"counterpart":"legacy@fake-1","unanswered":0,
 "clusters":[{"rows":[{"table":"size_of","row":1},{"table":"base_rate","row":6},{"table":"fuel_rate","row":2},{"table":"signature_fee","row":1}],"count":10,
              "delta":{"fee":{"min":-1,"max":-1,"uniform":true,"total":-10}},
              "witness":{"in":{"dest":"overseas","girth":1,"signature":true,"weight":1},"ours":{"fee":22},"theirs":{"fee":23}},
              "records":[{"line":10,"tag":""},{"line":65,"tag":""},…],
              "suspect_rounding":false},…],
 "moved":[],"refused":[],"excluded":{},"filled":{"count":0,"by_field":{},"defaults":{}},"cases":null}
```

The same shape for all three commands, defined in
[Formats](formats.md#verify-replay-diff).

---

[For agents](agents.md){ .md-button .md-button--primary }
[Formats](formats.md){ .md-button }
