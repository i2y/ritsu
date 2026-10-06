# The sakai language

sakai writes the part of a context map that can be held to the artifacts: which bounded context owns each file, which relationships the contexts have, and how a layer maps the words and values that cross.
Everything here is checked by `sakai check`; what it does not check is listed at the end.
The examples are from the example of this repository, `examples/shop` (in the repository of ritsu, crates/sakai), and from the maps the tests use.

## Two kinds of file

A `.ctx` file is a map or a context; its first word says which.

- A **map** gathers the context files, says which files it covers, and where the code of each language is.
- A **context** is one bounded context: its description and owner, the files and directories it owns, its published languages, its glossary, its relationships to other contexts, and the mappings of its anticorruption layers.

A context file belongs to the team of that context: its glossary and its mappings are written and reviewed there.
The map holds only what belongs to no context.

The sections come in a fixed order (E004).
A map: the heading, `description`, `use context`, `covers`, `except`, `proto root`, `code`.
A context: the heading, `description`, `owner`, `also`, `owns`, `published language` (any number), `terms`, then the relationships, in any order.
`use context`, `owns` and `covers` are required.
Indentation is spaces (a tab is E005); `#` starts a comment that runs to the end of the line.
A path is read from the directory of the file it is written in, as koyomi's `use calendar`, rulec's `import proto` and dandori's `use` are.

## The map

```ctx
map Shop(shop) v1
description "A small online shop that takes orders, holds stock, delivers and bills"

use context "contexts/ordering.ctx"
use context "contexts/inventory.ctx"
…
covers "."
proto root "proto"

code python "py"
code typescript "ts"
code java "java/src/main/java"
  test "java/src/test/java"
code go "go"
```

- The heading is `map <name>(<alias>) v<n>`. The alias is ASCII (`[A-Za-z_][A-Za-z0-9_]*`) and required (E008): it names the map in CML and in the settings `build` writes. The version is for people; it is copied into `api` and the heads of what sakai writes.
- `use context "<file>"`: a context of the map.
- `covers "<dir>"` and `except "<dir>"`: the files the map covers. Every artifact under `covers` and outside `except` must belong to exactly one context. A path with a part that starts with `.` (`.git`, `.venv`), and `node_modules`, `site-packages`, `__pycache__` and `target`, are never covered.

```ctx
except "py/billing"
```

- `proto root "<dir>"`: where the imports of the `.proto` files are looked up.
- `code <language> "<dir>"`, with `test "<dir>"` under it for Java: where the code of a language is, for `build` and for the artifacts of that language. The languages are `python`, `typescript`, `java`, `go` and `rust`; for Rust, the directory is the Cargo workspace.

```ctx
code rust "."
```

## A context

```ctx
context Inventory(inventory) v1
description "Keeps the counts on the warehouse shelves, holds stock for each line of an order, and answers how packing is going"
owner "Warehouse team"
also "stock control"
```

The heading is `context <name>(<alias>) v<n>`, with the alias required as for a map.
Names (of maps, contexts, terms and downstream values) can be written in any script; keywords are English only.
In a map, no two contexts share a name or an alias; in a context, no two terms share a name or an `also` (E006).

### What a context owns

```ctx
owns
  dir "../inventory", "../proto/warehouse"
  dir "../py/inventory", "../py/warehouse", "../ts/inventory", "../ts/warehouse"
```

`owns` lists directories (`dir "…"`) and files, a file named by the tool that reads it (`rulec "…"`, `koyomi "…"`, `dandori "…"`, `chobo "…"`, `proto "…"`), or for an OpenAPI or AsyncAPI document by its kind (`openapi "…"`, `asyncapi "…"`).
An artifact belongs to the context that wrote the deepest entry holding it; an entry that names the file is deeper than any directory.
That lets a map give most of a repository to one context and carve a directory out of it for another.
An artifact no entry holds is E101; two contexts writing entries of the same depth for it is E102; an entry that holds no artifact is W101.

These files are artifacts: `.rule`, `.flow`, `.cal`, `.book`, `.geas`, `.proto`, the OpenAPI and AsyncAPI documents (a `.yaml`, `.yml` or `.json` whose top holds `openapi`, `asyncapi` or `swagger`) with the files of YAML or JSON they reach by `$ref`, and the code of a language the map declares, under its directory (`.py`; `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs`; `.java`; `.go`; `.rs` and a crate's `Cargo.toml`).
A `.ctx` is not.
What an artifact reads as part of itself (a calendar's table of holidays, a copy of a law, a Smithy model) does not need an owner.
A `.proto` that is distributed elsewhere (`google/protobuf/…`, `buf/validate/…`, dandori's `dandori/v1/options.proto`) is not an artifact, even when a copy is in the repository.

### Published language and open host service

```ctx
published language warehouse.v1
  proto "../proto/warehouse/v1/stock.proto"
  open host service StockService, PackingService
  generated dir "../py/warehouse/v1", "../ts/warehouse/v1"
  generated dir "../java/src/main/java/warehouse/v1", "../go/warehouse/v1"
```

```ctx
published language rulec.urgency.v1
  rulec "../delivery/rules/urgency.rule"
  open host service UrgencyService
```

A published language is a protobuf package, written as a block `published language <package>`:

- `proto "…"`: a file that declares the package and belongs to the context (else E302).
- `rulec "…"`: a rule, whose Connect service (the one `rulec gen` writes for it) is the published language; its package is the one of the rule's Connect path (`rulec.urgency.v1`).
- `open host service <service>, …`: the services anyone may call; each must be in the block's files (else E301).
- `generated dir "…"`: where the code generated from the package is kept; other contexts' code may import it (see [targets.md](targets.md)).
- `crate "…"`: a Rust crate, whose name with `-` as `_` is the heading; a crate has no open host service.

```ctx
  crate "../stock"
```

What a published language holds is everything its `.proto` files define (messages and their fields, enums and their values, services and their methods), or, for a rule, what rulec names of it (inputs, outputs, enums and values).
A reference that calls a service needs the service to be an open host service; a reference that only uses types needs the published language.

```ctx
published language payments.v1
  openapi "../payments/api/payments.yaml"
  asyncapi "../payments/events/payments.yaml"
  open host service createCharge, getCharge, paymentSucceeded, paymentFailed
```

A published language may be OpenAPI and AsyncAPI documents (`openapi "…"`, `asyncapi "…"`, any number of each), which mix with no `.proto`, rule or crate (E004).
sakai reads OpenAPI 3.0, 3.1 and 3.2, and AsyncAPI 3.0 and 3.1, in JSON or in YAML; of YAML, what goes to JSON and back (RFC 9512, section 3.4), and E108 stops at the rest, at OpenAPI 2.0 and at AsyncAPI 2.x.
A server of a document that does not encrypt the connection to a host off the machine is W902, unless the server says why in `x-ritsu-plaintext: "<why>"`; an operation of a published language with no `security` (nor its document's), or an AsyncAPI channel on a server with none, is W903, unless it is open to anyone on purpose and says so with `security: []`.
The heading is the name the context gives the language, as a document has no package; no two published languages share one (E006).
Its open host services are the HTTP operations, by their `operationId` (or, with none, a method and a path in quotes, `"GET /orders"`), and the channels, by their keys under `channels`, that anyone may use (each must be in the block's documents, else E301).
A part of a document that a listed document reaches by `$ref`, and that belongs to the same context, is in the published language too.

### Terms

```ctx
terms
  reservation "Holding shelf stock for one line of an order until it ships or is cancelled"
    means message ReserveResponse
  out_of_stock "The count asked for is not on the shelf"
    means enum Stock value STOCK_SHORT
```

A term has a name and a definition, and `also` for other names of the same thing.
`means` names an element of the context's own published language that the term stands for (E408 otherwise); a `.proto` element can be named by its name in the package.
A term with `means` crosses the boundary with its element.
`as <context>.<term>` takes a term of a context it has a relationship with, with the same meaning, and no definition of its own (E410 when there is no such term or no relationship).

```ctx
  reservation as Inventory.reservation
```

A term that crosses no boundary is W401.
When a term of an upstream crosses into a downstream whose glossary has a term of the same name (not taken with `as`), the two are the same word in two meanings: the downstream's anticorruption layer must map what crosses (E406), and not to a name that is its own term's (E407).
sakai does not decide whether two definitions mean the same: only `as` says so.

### Relationships

```ctx
upstream Inventory anticorruption layer
  through warehouse.v1
  layer dir "../py/delivery/acl/inventory", "../ts/delivery/acl/inventory"
…
downstream Billing supplier

shared kernel with Billing
  koyomi "../calendars/tokyo_business_days.cal"
  dir "../py/calendars", "../ts/calendars", "../java/src/main/java/calendars", "../go/calendars"

partnership with Ordering
```

```ctx
separate ways from Billing
```

- **Upstream and downstream** are written by the downstream: `upstream <context> <role>`, the role `conformist`, `anticorruption layer` or `customer` (a customer can add an anticorruption layer: `customer, anticorruption layer`). `through` lists the packages of the upstream's published language the relationship goes through, and is required. An anticorruption layer can say where the layer is (`layer`) and map enums and terms.
- **Customer and supplier** needs both sides: `upstream … customer` and the supplier's `downstream … supplier` (E303 for one side only).
- **Shared kernel** is written by both contexts, each listing the same files and directories (E307 when the lists differ).
- **Partnership** is written by both (E309 for one side only).
- **Separate ways** is written by either; a reference between the two is E206, and any other relationship between them E310.

```ctx
upstream Pricing customer, anticorruption layer
```

What each relationship lets cross a boundary:

| Relationship | What may cross |
|---|---|
| conformist | from the downstream to the elements of the packages in `through`; the downstream's published language may use the upstream's types |
| anticorruption layer | the same, but only from inside `layer` when it is written, and the downstream's published language must not show the upstream's types (E205) |
| customer | as conformist, once the supplier has agreed (`downstream … supplier`) |
| shared kernel | from either side to the listed files and directories |
| partnership | from either side to the elements of the other's published language |
| separate ways | nothing |

A reference that calls a service (dandori's `connect`, `use rule … connect`) also needs the service to be an open host service of the other context (E207).
A flow runs a child flow across a boundary only in a partnership, a shared kernel, or when the child implements the other's open host service (E209).
A rule bundled, deployed or applied across a boundary is a use of the rule itself, and is allowed only inside a shared kernel (E202).

### Mappings in an anticorruption layer

```ctx
  enum PackingStatus -> shipping_decision
    PACKING_STATUS_WAITING -> wait
    PACKING_STATUS_PACKED  -> ship
    PACKING_STATUS_SHORT   -> refuse "A box with an item missing is not shipped. It goes back to ordering"
```

```ctx
  enum OrderStatus -> rulec "../billing/rules/billing_need.rule" enum order_status
```

```ctx
  term dispatch -> cancel
```

`enum <upstream enum> -> <target>` maps each value of the upstream's enum, read from its `.proto`, to a value downstream or to `refuse` (with a reason).
The upstream enum may be a schema with `enum` under `components/schemas` of the upstream's OpenAPI or AsyncAPI documents: its values are the strings as they are written (in any case, no prefix taken off; a value with a blank in it is written in quotes, `"in transit" -> …`), and a `null` among them marks that nothing is set (W402 if a line is written for it).

```ctx
  enum ChargeStatus -> enum ShipmentGate
    pending   -> hold
    succeeded -> release
    failed    -> refuse "An order whose charge failed is not shipped"
    refunded  -> refuse "A refunded order is not shipped"
```

- A value of the upstream with no line is E401, naming every missing value: that is what an upstream adding a value meets. A line for a value the enum does not have is E402.
- The value 0 whose name, without the enum's prefix, is `unspecified` marks that nothing is set and needs no line (W402 if one is written), as with rulec's `import proto` and dandori's types from a `.proto`. Any other value 0 is a value like the rest.
- The target is one of three: a rule's enum (`rulec "…" enum …`), where a rule that takes the upstream's enum in with `import proto` is the mapping itself (lines written beside it must agree, E405; rulec keeps it complete with its E032 and E033), and a rule that does not needs value lines whose values are the rule's (E403); a downstream enum of a `.proto` or of the context's own documents, whose values the lines must use (E403); or only a name, whose values are taken as written and not checked (`doc` says so).
- `term <upstream term> -> <downstream term>` says which downstream term receives an upstream one; both glossaries must have it (E409).
- An upstream enum that the downstream's artifacts refer to, with no mapping, is E404.

Only an anticorruption layer writes mappings (E304, E305).

## Naming an artifact

A context file names a file or a thing in it in one form, the same in yuen and in ritsu (`ritsu-base/tests/fixtures/naming.tsv` is its table of tests):

```text
<tool> "<path>" [<kind> <name>]...
```

The tools are `rulec`, `dandori`, `koyomi`, `chobo`, `geas`, `proto`, `openapi`, `asyncapi`, `cedar`, `file`, `yuen` and `sakai`.
A kind can hold another only as its tool nests them: `service S method M`, `message M field f`, `enum E value V` for a `.proto`, `enum E value V` for a rule, `transfer T operation O` for a book, `schema S property P` and `schema S value V` for an OpenAPI or AsyncAPI document, `channel C message M` for an AsyncAPI one.
The kinds are those of each tool: rulec's `input`, `output`, `enum`, `value`, `table`, `clause`, `define`, `derive`, `machine`, `source`; koyomi's `input`, `date`, `claim`, `source`; chobo's `unit`, `account`, `transfer`, `operation`; geas's `claim`; dandori's `task`, `case`, `record`, `field`, `enum`, `value`, `input`, `output`; the `.proto`'s `service`, `method`, `message`, `field`, `enum`, `value`; OpenAPI's `schema`, `property`, `value`, `operation`, `pointer`; AsyncAPI's `channel`, `message`, `operation`, `schema`, `property`, `value`, `pointer`; Cedar's `policy`, `action`, `entity`; yuen's `requirement`, `source`; sakai's `context`, `term`.
A name is a word (no space, `"` or `#`) or a string in `"…"` with `\"` and `\\` as its only escapes.
A path is from the directory of the file it is written in, `/` between its parts; an absolute path, or one that leaves the root, is E012.
In JSON, a path is from the root: the nearest directory above the first path given that holds `.git`, or `--root`.

The tool words and the kind words are keywords: no map, context, term or downstream value can be named with one (E002).

The elements of an OpenAPI or AsyncAPI document are named in this form too: `openapi "payments/api/payments.yaml" schema Charge`, `openapi "…" operation createCharge` (an `operationId`, or for an operation with none its method and path, `"POST /charges"`), `asyncapi "…" channel paymentFailed`, and anything else by its JSON Pointer, `openapi "common/money.yaml" pointer /Money`. `sakai api` and the diagnostics write them so.
In a context file they can also be written short, in the context's own published language (`means`, the target of a mapping) or in the upstream's (the enum of a mapping): `schema <name>` and `enum <name>` (with `value <value>` under it) for `components/schemas`, `message <name>`, `channel <name>`, and `operation <name>` (an `operationId`, or a key of AsyncAPI's `operations`).
A `cedar` file is not an artifact sakai reads, and cannot be written under `owns` (E002).

## What `check` checks, in order

1. The words, the sections, the names, the aliases and the paths (E001 to E012), and the keys written in the map and its context files (W901).
2. Who owns what: every artifact covered belongs to exactly one context (E101 to E103, W101, W103).
3. What the artifacts say: the `.proto` files, what rulec, koyomi and dandori answer of their files through ritsu's ports, the crates of the Rust code as Cargo says them, the OpenAPI and AsyncAPI documents and their `$ref`s, with their servers and how they ask a client to prove who it is; then the elements the map names (E104 to E108, W102, W104, W902, W903, E007, E011).
4. The patterns agree with each other (E301 to E313, W301).
5. The references that cross a boundary (E201 to E210): a document's `$ref` to another context's, and an AsyncAPI operation on another context's channel, among them.
6. The mappings and the glossaries (E401 to E410, W401, W402).

An error in stage 1 or 2 stops what follows; from stage 3 on every stage runs on what could be read.
Each code, with when it comes, how to fix it and the smallest reproduction, is in [codes.md](codes.md) (and `sakai explain <code>`).

## What is not checked

- Whether the code of an anticorruption layer maps as its mapping says.
- Calls no contract writes: HTTP to a URL in a string with no OpenAPI document, a queue with no AsyncAPI document, a shared database, reflection and dynamic imports. What OpenAPI and AsyncAPI documents write is checked.
- Whether the code calls HTTP, and sends to and receives from channels, as its documents say.
- Whether generated code really was generated from its published language.
- What a definition says.
- The imports of the code: the settings `sakai build` writes have the import linters check them ([targets.md](targets.md)). Rust is the exception: `check` holds a crate's dependencies to the map itself, as Cargo states them.

## The keywords

Every keyword has one English spelling and no synonym.

| Where | Keywords |
|---|---|
| map file | `map`, `description`, `use context`, `covers`, `except`, `proto root`, `code`, `python`, `typescript`, `java`, `go`, `rust`, `test` |
| context file | `context`, `description`, `owner`, `also`, `owns`, `dir`, `published language`, `crate`, `openapi`, `asyncapi`, `open host service`, `generated dir`, `terms`, `means`, `as` |
| relationship | `upstream`, `downstream`, `conformist`, `anticorruption layer`, `customer`, `supplier`, `through`, `layer`, `enum`, `term`, `refuse`, `shared kernel with`, `partnership with`, `separate ways from` |
| tool | `rulec`, `dandori`, `koyomi`, `chobo`, `geas`, `proto`, `openapi`, `asyncapi`, `cedar`, `file`, `yuen`, `sakai` |
| kind | `input`, `output`, `enum`, `value`, `table`, `clause`, `define`, `derive`, `machine`, `source`, `date`, `claim`, `unit`, `account`, `transfer`, `service`, `method`, `message`, `field`, `requirement`, `context`, `term`, `task`, `case`, `record`, `schema`, `channel`, `operation`, `property`, `pointer`, `policy`, `action`, `entity` |

## Commands

| Command | What it does |
|---|---|
| `sakai check <map.ctx \| dir>... [--format json] [--root <dir>]` | check maps; a directory stands for every map under it |
| `sakai build <map.ctx> --target <tool> [--out <dir>] [--check]` | write the settings of an import linter from the map, or check that they are up to date ([targets.md](targets.md)) |
| `sakai export cml <map.ctx> [--out <file>]` | write the map as Context Mapper's CML |
| `sakai doc <map.ctx> [--format markdown\|html] [--out <dir>]` | write the map's page: the context map, each context's artifacts and glossary, the relationships and what crosses them, the mappings |
| `sakai api <map.ctx>` | print the map, the owners and the crossings as JSON |
| `sakai explain <code>`, `sakai explain --all [--format markdown]` | look a diagnostic up |

Every command takes `--lang ja|en` (else `SAKAI_LANG`, then `RITSU_LANG`, else English).
The exit code is 0 for no errors, 1 for errors, and 2 for bad arguments, a file that cannot be read, or an artifact of a language that is not joined (E104).
`ritsu sakai …` (or `sakai`, the link of that name to ritsu) joins every language sakai reads; the binary of sakai's own crate joins none, and stops with E104 at a map that holds rules, calendars or workflows.
