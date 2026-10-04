# The yuen namespace

`https://i2y.github.io/ritsu/ns/yuen#` is the namespace of the words yuen adds to
[W3C PROV](https://www.w3.org/TR/prov-overview/) when it writes the provenance of a project with
`yuen export prov`, as [PROV-N](https://www.w3.org/TR/prov-n/), or as
[PROV-JSON](https://www.w3.org/submissions/prov-json/) with `--format json`. A document writes it
as the prefix `yuen`, so `yuen:Requirement` is the IRI `https://i2y.github.io/ritsu/ns/yuen#Requirement`,
and opening that address brings you to its entry below.

The other prefix a document declares, `y`, is `urn:yuen:`. It names the things themselves
(`y:requirement/extinguisher_distance/v1`, `y:role/safety`) and is not an address: there is
nothing to open.

What yuen writes is what the `.req` files say, and the words here are only the ones PROV has no
word for. The rest is PROV's own: `entity`, `agent`, `activity`, `used`, `wasAssociatedWith`,
`wasAttributedTo`, `wasInfluencedBy`, `wasDerivedFrom` (with `prov:type='prov:PrimarySource'` for
a requirement read from an article, and `'prov:Revision'` for a version after another, or a
requirement that replaces another), and `prov:label`, the name of a thing.

For example, yuen's `osha` example, as `ritsu yuen export prov` writes it (every line is the
command's; `…` stands for the lines left out):

```console
$ cd crates/yuen
$ ritsu yuen export prov examples/osha/osha.req --root examples/osha
document
  prefix yuen <https://i2y.github.io/ritsu/ns/yuen#>
  prefix y <urn:yuen:>

  agent(y:role/safety, [prov:type='yuen:Role', prov:label="safety", yuen:description="Decides how the regulation reads"])
…
  entity(y:source/7e4bbf45001c614fa29502d3bc2fa12e, [prov:type='yuen:Source', prov:label="osha §1910.157", yuen:law="ecfr 29 CFR 1910", yuen:asof="2026-01-01", yuen:sha256="c2a9ce966c7e2269"])
  entity(y:artifact/771f0952ef8ca8e6321199853c71e166, [prov:type='yuen:Artifact', prov:label="rulec \"osha_extinguisher.rule\" table distance", yuen:sha256="5681500476af8a56", yuen:end="item"])
…
  entity(y:requirement/extinguisher_distance/v1, [prov:type='yuen:Requirement', prov:label="extinguisher_distance", yuen:version="1", yuen:text="Portable fire extinguishers for Class A hazards are placed so that the travel distance to any extinguisher is 75 feet or less, and for Class B hazards 50 feet or less", yuen:inForce="2026-01-01..", yuen:sha256="849dc53f1d112c23"])
  wasAttributedTo(y:requirement/extinguisher_distance/v1, y:role/safety, [prov:type='yuen:owner'])
  wasDerivedFrom(y:requirement/extinguisher_distance/v1, y:source/7e4bbf45001c614fa29502d3bc2fa12e, -, -, -, [prov:type='prov:PrimarySource'])
  wasInfluencedBy(y:artifact/771f0952ef8ca8e6321199853c71e166, y:requirement/extinguisher_distance/v1, [prov:type='yuen:satisfies'])
  wasInfluencedBy(y:artifact/682980cd3f5c2ef23bcf0200bf205bf9, y:requirement/extinguisher_distance/v1, [prov:type='yuen:verifies'])
  activity(y:review/e726e58c2231b4f0d6b32da28a1db548, 2026-10-04T00:00:00, -, [prov:type='yuen:Review', yuen:link="from", yuen:up="c2a9ce966c7e2269", yuen:down="849dc53f1d112c23", yuen:status="ok"])
  used(y:review/e726e58c2231b4f0d6b32da28a1db548, y:source/7e4bbf45001c614fa29502d3bc2fa12e, -)
  used(y:review/e726e58c2231b4f0d6b32da28a1db548, y:requirement/extinguisher_distance/v1, -)
  wasAssociatedWith(y:review/e726e58c2231b4f0d6b32da28a1db548, y:role/safety, -)
…
  wasInfluencedBy(y:artifact/682980cd3f5c2ef23bcf0200bf205bf9, y:source/7e4bbf45001c614fa29502d3bc2fa12e, [prov:type='yuen:pins'])
endDocument
```

## How the words are written

- A **type** is the value of `prov:type` of an `entity`, an `agent` or an `activity`:
  `prov:type='yuen:Requirement'`.
- A **relation** word is the value of `prov:type` of a `wasAttributedTo` or a `wasInfluencedBy`, and
  says what sort of relation it is: `[prov:type='yuen:satisfies']`.
- An **attribute** is a name in the list of attributes of a record: `yuen:version="1"`. Its value
  is a string, whether it is a number, a date or a hash.

In PROV-JSON, a type and a relation word are qualified names
(`{"$": "yuen:Requirement", "type": "prov:QUALIFIED_NAME"}`), and an attribute is a key that holds a
string.

## Types { #types }

### Requirement { #Requirement }

`yuen:Requirement` · a type of [`entity`](https://www.w3.org/TR/prov-dm/#concept-entity)

One version of a requirement, as a `requirement` of a `.req` declares it; a requirement with two
versions is two of these. Its `prov:label` is its name. Its attributes are [`version`](#version),
[`text`](#text), [`inForce`](#inForce) and [`sha256`](#sha256). Its owner is a [Role](#Role)
([`owner`](#owner)); it comes from [Sources](#Source) and from other requirements; each
[Artifact](#Artifact) that meets it or checks it is influenced by it ([`satisfies`](#satisfies),
[`verifies`](#verifies)).

### Source { #Source }

`yuen:Source` · a type of [`entity`](https://www.w3.org/TR/prov-dm/#concept-entity)

What requirements are read from: one article of a law as of a date, or a whole file, as a `source`
of a `.req` names it. A requirement that cites it is derived from it
(`prov:type='prov:PrimarySource'`). Its `prov:label` is the source's name and the article
(`osha §1910.157`). An article has [`law`](#law), [`asof`](#asof) and, when its copy says,
[`revision`](#revision); a file has [`file`](#file) and, when the `.req` gives one, [`url`](#url).
Both have [`sha256`](#sha256), the hash the copy is pinned by.

### Artifact { #Artifact }

`yuen:Artifact` · a type of [`entity`](https://www.w3.org/TR/prov-dm/#concept-entity)

What meets a requirement or checks it, as a `satisfied by` or a `verified by` names it: a whole
file, or one thing in the file of another language (a table of a rule, a date of a calendar, a
claim). Its `prov:label` is that naming as the `.req` writes it:
`rulec "osha_extinguisher.rule" table distance`. Its attributes are [`sha256`](#sha256) and
[`end`](#end).

### Role { #Role }

`yuen:Role` · a type of [`agent`](https://www.w3.org/TR/prov-dm/#concept-agent)

A role a project declares with `role`: a person or a department. It owns requirements, decides,
looks at links and approves waivers. Its `prov:label` is its name, and its attribute is
[`description`](#description).

### Decision { #Decision }

`yuen:Decision` · a type of [`activity`](https://www.w3.org/TR/prov-dm/#concept-activity)

What `decided` writes under a requirement: who decided what the requirement says or how its source
reads, when, and why. The activity starts on that day, is associated with the role that decided,
and used the sources the requirement cites. The requirement is influenced by it, not generated by
it, because a decision can be added as long as the requirement lives. Its attribute is
[`why`](#why).

### Review { #Review }

`yuen:Review` · a type of [`activity`](https://www.w3.org/TR/prov-dm/#concept-activity)

One look of a person at one link, as `yuen review` records it under the link
(`reviewed 2026-10-04 by safety sha256:… -> sha256:…`). The activity starts on the day of the look,
is associated with the role that looked, and used both ends of the link. Looking again is another
Review, and a link nobody has looked at has none. Its attributes are [`link`](#link), [`up`](#up),
[`down`](#down) and [`status`](#status).

### Waiver { #Waiver }

`yuen:Waiver` · a type of [`activity`](https://www.w3.org/TR/prov-dm/#concept-activity)

The decision to leave a requirement without what meets it (`not satisfied`) or without what checks
it (`not verified`), with the reason. It used the requirement. Once a person has approved it with
`approved`, the activity starts on the day of the approval, is associated with that role, and holds
the hash of the requirement as it was then; until then it has no start, and its status is
`unapproved`. Approving again is another Waiver. Its attributes are [`side`](#side),
[`why`](#why), [`sha256`](#sha256) and [`status`](#status).

## Relations { #relations }

### owner { #owner }

`yuen:owner` · a sort of [`wasAttributedTo`](https://www.w3.org/TR/prov-dm/#concept-attribution)

`wasAttributedTo(requirement, role, [prov:type='yuen:owner'])`: the role that owns the requirement,
as its `owner` line says. It is the role `yuen affected` names when a change reaches the
requirement.

### satisfies { #satisfies }

`yuen:satisfies` · a sort of [`wasInfluencedBy`](https://www.w3.org/TR/prov-dm/#concept-influence)

`wasInfluencedBy(artifact, requirement, [prov:type='yuen:satisfies'])`: the artifact meets the
requirement. There is one for each `satisfied by` line.

### verifies { #verifies }

`yuen:verifies` · a sort of [`wasInfluencedBy`](https://www.w3.org/TR/prov-dm/#concept-influence)

`wasInfluencedBy(artifact, requirement, [prov:type='yuen:verifies'])`: the artifact checks the
requirement. There is one for each `verified by` line.

### pins { #pins }

`yuen:pins` · a sort of [`wasInfluencedBy`](https://www.w3.org/TR/prov-dm/#concept-influence)

`wasInfluencedBy(file, article, [prov:type='yuen:pins'])`: the file of an artifact (a rule, a
calendar) pins that article of a law: it holds the hash of the same copy the project reads. A
language pins file by file, so it is the whole file that is influenced. When no link names the
file whole, the file is written as an [Artifact](#Artifact) with [`end`](#end) `file`, for this.

## Attributes { #attributes }

### version { #version }

`yuen:version` · an attribute of [`Requirement`](#Requirement)

The number of the version, as a string: `"1"`. A version is derived from the one before it
(`prov:type='prov:Revision'`).

### text { #text }

`yuen:text` · an attribute of [`Requirement`](#Requirement)

The sentence of the requirement, as `text` writes it. yuen does not read what it means; it checks
the links and the hashes around it.

### inForce { #inForce }

`yuen:inForce` · an attribute of [`Requirement`](#Requirement)

The period the version is in force, as `in force` writes it: `2026-10-01..` (from that day on),
`..2027-03-31` (until that day) or `2026-10-01..2027-03-31`. It is there only when the `.req` says.

### law { #law }

`yuen:law` · an attribute of [`Source`](#Source)

The database and the law an article is of, as the `source` line says: `ecfr 29 CFR 1910` (the
US eCFR) or `egov 129AC0000000089` (Japan's e-Gov).

### asof { #asof }

`yuen:asof` · an attribute of [`Source`](#Source)

The day the article is read as of, as the `source` line says: `2026-01-01`.

### revision { #revision }

`yuen:revision` · an attribute of [`Source`](#Source)

The revision of the law the copy was taken from, when the copy records it (e-Gov's identifier, such
as `129AC0000000089_20260624_508AC0000000045`).

### file { #file }

`yuen:file` · an attribute of [`Source`](#Source)

The path of the file a `file` source pins, from the root of the project.

### url { #url }

`yuen:url` · an attribute of [`Source`](#Source)

Where the file of a `file` source is fetched from, when the `.req` says.

### end { #end }

`yuen:end` · an attribute of [`Artifact`](#Artifact)

`file` when the artifact is a whole file, `item` when it is one thing in a file.

### description { #description }

`yuen:description` · an attribute of [`Role`](#Role)

What the role decides, as the `role` line says.

### why { #why }

`yuen:why` · an attribute of [`Decision`](#Decision) and [`Waiver`](#Waiver)

The reason, as a person wrote it. yuen does not read it.

### side { #side }

`yuen:side` · an attribute of [`Waiver`](#Waiver)

What is left out: `not satisfied` or `not verified`.

### link { #link }

`yuen:link` · an attribute of [`Review`](#Review)

The sort of link that was looked at: `from`, `satisfied by` or `verified by`.

### up { #up }

`yuen:up` · an attribute of [`Review`](#Review)

The hashes of the upper ends of the link, as the person saw them, separated by spaces: the articles
or the requirement a `from` line reads, or the requirement for a `satisfied by` or a `verified by`.

### down { #down }

`yuen:down` · an attribute of [`Review`](#Review)

The hash of the lower end of the link, as the person saw it: the requirement for a `from` line, the
artifact for the others.

### status { #status }

`yuen:status` · an attribute of [`Review`](#Review) and [`Waiver`](#Waiver)

How the record compares with the project as it is now, in the words of `yuen api`: `ok` (every end
is as it was looked at), `up_changed` (an upper end, or the requirement a waiver was approved on,
has changed), `down_changed` (the lower end has changed), `unapproved` (a waiver no one has
approved), `bad_record` (the record is not written right) and `unreadable` (an end could not be
made, so nothing was compared).

### sha256 { #sha256 }

`yuen:sha256` · an attribute of [`Requirement`](#Requirement), [`Source`](#Source),
[`Artifact`](#Artifact) and [`Waiver`](#Waiver)

A hash of what the thing is, written as the first 16 hexadecimal digits of its SHA-256. For a
source, the copy of the article or of the file: the value the `.req` pins it by. For an artifact,
what the link names, as its language reads it (a table, a date, or the bytes of a file). For a
requirement, its text, what it is read from and its period. For a waiver, the requirement's hash
at the approval.

## Where the list comes from

The words are the list `TERMS` in
[`crates/yuen/src/export/prov.rs`](https://github.com/i2y/ritsu/blob/main/crates/yuen/src/export/prov.rs).
ritsu's tests hold this page to it: every word of the list is here once, under the heading of its
kind, with the types it is written on, and the example above is what `ritsu yuen export prov`
prints. yuen's tests hold the list to what `yuen export prov` writes for every project they have.

See also [yuen](https://github.com/i2y/ritsu/blob/main/crates/yuen/README.md), its
[reference](https://github.com/i2y/ritsu/blob/main/crates/yuen/docs/reference.md), and
[ritsu](../index.md).
