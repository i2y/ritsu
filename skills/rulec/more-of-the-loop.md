# More of the loop

The steps of rulec's loop (section 1 of SKILL.md) that are for one occasion each. They are here
rather than in SKILL.md, which is read whole whenever the skill is used.

## In CI

```yaml
- uses: actions/checkout@v7                  # with fetch-depth: 0, so --diff-base can read origin/main
- uses: i2y/ritsu@v0.38.0                     # ritsu's release with the link rulec, verified against its checksums
- run: rulec fmt --check rules/
- run: rulec check rules/ --diff-base origin/main
- run: rulec gen rules/ --out generated/ --check
- run: rulec coverage rules/
- run: rulec test generated/ --require-all
```

`--diff-base` reports only findings that are new since that revision, so pre-existing
warnings do not drown the ones this change introduced. `gen --check` fails when the generated
code is stale — that is the gate that keeps the committed output honest.

## If there is a legacy implementation: `rulec verify`

Write a 20-to-30-line adapter (`rulec adapter --template python|go` gives you the shape) and
rulec streams the vectors through it. Mismatches come back clustered by the rows that fired,
with counts, amount differences and a witness; a cluster whose differences are all smaller
than the output's rounding grid is flagged as a suspected rounding difference.

**A mismatch is not automatically your bug.** It is one of four things: a defect in the legacy
implementation, a transcription error in your table, dirty records, or a rounding convention.
The cluster and its witness are what tell them apart.

## Comparing two versions: `rulec diff`, with records and without

**With no `--fixtures`** it answers what a log cannot: which inputs get a different answer, as
a region in the rule's own columns, plus the claim that there are none outside — withheld when
it was not earned. Run it first: the region is what the whole rule does with the change, not
what the changed row says (a base fee a later table multiplies by 0% moves nobody). **With
`--fixtures`** it answers how many of your records move and by how much, read as the old version
reads them; inputs the new one does not take are listed apart. Run `fixtures lint` first, always.

A version is a file, a git tag (`parcel@v3` is `rules/parcel/v3`, else the revision `v3`), a path at a revision (on a
PR, `rules/parcel.rule@origin/main`), or a dependency's rule: `@tax/vat.rule` at the lock's version, `@tax@1.0.0/vat.rule` from the
store. `--format markdown` is what gets posted and `--terse` keeps record values out; [formats.md](formats.md) has both shapes.

## For people: `rulec doc`

`rulec doc <file> --lang ja` renders the page for people: the rule in markdown with the facts the
checker knows that the text does not show — that a group of six values and its complement of 41
really do cover all 47, which rows shadow which, where a rounding was assumed rather than sourced.
`--format html` renders the same page with a form on it: a reader types a case, the rows that
matched light up, the outputs appear, and the line the generated code would log is shown — it is
the generated JavaScript itself that runs, so the page says nothing the code does not. Either is
rendered in CI per change and pasted into the PR, never committed: a stale rendering that still
looks authoritative is the danger it is designed against.

## For the customer: `rulec doc --audience customer`

`rulec doc <file> --audience customer` renders the rule as the article a help centre
publishes, so that whatever answers customers — a person, a search, a model reading retrieved
pages — reads a page that is complete, carries the version, and spells out both sides of every
threshold. Those come from the boundary-pair vectors: the article says "girth 60cm → fee 1410JPY; 61cm → fee
1710JPY" where the table says `<=60cm`, and aliases, ranges and codes are left out. Regenerate it
on every change; like the page for people, it is a product of the file, never a source.
