---
name: sakai
description: Write, check and fix sakai files (`.ctx`), the checkable part of a context map — which bounded context owns each artifact, which relationships (conformist, anticorruption layer, customer and supplier, shared kernel, partnership, separate ways) let a reference cross a boundary, and how an anticorruption layer maps the enum values and terms that cross — held to real rules, workflows, calendars, books, `.proto` files, OpenAPI and AsyncAPI documents and code. Use when a context map has to be written or changed as `.ctx` files; when services that talk over HTTP or events have to be held to their OpenAPI and AsyncAPI contracts; when a sakai diagnostic (E001-E502, W101-W402) has to be fixed; when the settings of import-linter, dependency-cruiser, ArchUnit or go-arch-lint have to be written from the map; or when the map's page has to be shown to the people who check what the code is to do.
compatibility: Requires the `ritsu` binary on PATH (`cargo install --git https://github.com/i2y/ritsu --locked ritsu`); run sakai as `ritsu sakai <command>`, or as `sakai <command>` through a link to ritsu named for it. The import linters themselves run in the project's CI, not in sakai.
license: MIT OR Apache-2.0
---

## When this applies

The job is a **context map that the code is held to**: which team's context owns which files, which context may use which other context's published language or call its services, and what an anticorruption layer does with each value that crosses.
sakai writes it as `.ctx` files beside the artifacts and checks every artifact against it.

It does not apply to what the code does inside a context, to the meaning of a definition, or to calls no contract writes (a URL in a string with no OpenAPI document, a queue with no AsyncAPI document, a shared database).
sakai checks none of those. The HTTP operations and the channels that OpenAPI and AsyncAPI documents write, it checks.

The files bundled with this skill are `reference.md` (the whole language and the commands), `targets.md` (the import linters and CML) and `codes.md` (every diagnostic).
Read them when you need them, not all up front.

---

# Working with sakai

Your part is to write the map, get it past `ritsu sakai check`, write the linters' settings and the page for people.
Two things stay with people:

1. **Where the boundaries are, and who owns each context.** Never invent a context, a relationship or an owner to make the check pass.
2. **What a value becomes across a boundary.** When an upstream adds a value and E401 stops the check, the downstream's team decides what it becomes, or that it is refused.

## 1. The loop

1. Read what is there: the directories, the `.proto` files, the rules, workflows, calendars and books. Each artifact must end up in exactly one context.
2. Write the map (`map`), one context file per context (`context`), then the relationships, each in the downstream's file.
3. Run the check (`ritsu sakai check`, or `ritsu check` for the whole project) and fix what it says, one diagnostic at a time, from the top; `ritsu sakai explain <code>` says how.
4. Write the settings of the import linters, and have CI run them with `ritsu sakai build … --check`.
5. Write the page for people with `ritsu sakai doc`, and show it to the people who check what the code is to do.

```console
$ ritsu sakai check examples/shop/shop.ctx
$ ritsu sakai explain E401
$ ritsu sakai build examples/shop/shop.ctx --target import-linter
$ ritsu sakai doc examples/shop/shop.ctx --format html --out site
```

Run every command here as `ritsu sakai <command>` (through a link to ritsu named sakai, `sakai <command>` is the same): it reads the rules, calendars and workflows a map refers to in the same process, where sakai built alone from its crate reads no other language and stops with E104.
Add `--lang ja` for Japanese.

## 2. The language on one page

A map:

```ctx
map Shop(shop) v1
use context "contexts/ordering.ctx"
covers "."
proto root "proto"
code python "py"
```

A context, in the order the sections must come (E004):

```ctx
context Inventory(inventory) v1
description "Keeps the counts on the warehouse shelves, holds stock for each line of an order, and answers how packing is going"
owner "Warehouse team"
owns
  dir "../inventory", "../proto/warehouse"
published language warehouse.v1
  proto "../proto/warehouse/v1/stock.proto"
  open host service StockService, PackingService
terms
  reservation "Holding shelf stock for one line of an order until it ships or is cancelled"
    means message ReserveResponse
```

Relationships, written by the downstream (customer and supplier, shared kernel and partnership by both):

```ctx
upstream Inventory conformist
  through warehouse.v1
upstream Inventory anticorruption layer
  layer dir "../py/delivery/acl/inventory", "../ts/delivery/acl/inventory"
  enum PackingStatus -> shipping_decision
    PACKING_STATUS_SHORT   -> refuse "A box with an item missing is not shipped. It goes back to ordering"
downstream Billing supplier
shared kernel with Billing
partnership with Ordering
separate ways from Billing
```

- Every map and context has an ASCII alias: `Inventory(inventory)`.
- A path is from the file it is written in.
- An artifact belongs to the context with the deepest `owns` entry that holds it.
- A term with `means` crosses with its element; `as <context>.<term>` takes another context's term with the same meaning.
- An enum mapping gives every upstream value a downstream value or `refuse`. Where a rule takes the enum in with `import proto`, point the mapping at the rule's enum and write no lines: the rule is the mapping.

Services that talk over HTTP and events publish their OpenAPI and AsyncAPI documents as a published language, and open the HTTP operations (by `operationId`) and the channels (their keys under `channels`) anyone may use:

```ctx
published language payments.v1
  openapi "../payments/api/payments.yaml"
  asyncapi "../payments/events/payments.yaml"
  open host service createCharge, getCharge, paymentSucceeded, paymentFailed
```

- A document is found by its top (`openapi`, `asyncapi`), in JSON or YAML, and belongs to one context like any artifact. sakai reads OpenAPI 3.0 to 3.2 and AsyncAPI 3.0 and 3.1; convert AsyncAPI 2.x with `asyncapi convert`.
- What crosses is what the documents write: a `$ref` to another context's document, and an operation that sends to or receives from another context's channel (the downstream writes `upstream`, as for any reference).
- Name a document's element short, in the context's own published language or the upstream's: `schema Charge`, `enum ChargeStatus`, `channel orderPlaced`, `message OrderPlaced`, `operation createCharge`. An enum's values are its strings, as written.

## 3. What to ask a person

- Which team owns a directory that no context owns yet (E101), or that two contexts both claim (E102).
- Whether a reference that crosses with no relationship (E201) is a dependency to declare, or a mistake to remove.
- Whether two terms of the same name mean the same (then `as`) or not (then rename, or map it in an anticorruption layer), when E406 says they meet.
- What a new upstream value becomes (E401). Do not write `refuse` without asking.
- Whether a relationship is customer and supplier: it needs the supplier's team to write `downstream … supplier` (E303).

## 4. From a diagnostic to a fix

`ritsu sakai explain <code>` (and `codes.md`) gives each code's cause, its fix and its smallest reproduction.
The usual ones:

| Code | What it means | What to do |
|---|---|---|
| E101, E102 | an artifact with no owner, or two | add or deepen an `owns` entry (ask who owns it) |
| E104 | a language is not joined | run it as `ritsu sakai` |
| E201 | a reference crosses with no relationship | declare the relationship, or remove the reference (ask) |
| E202 | a reference to the inside of another context | use what the other context publishes, or publish what is used |
| E203 | a reference through a package `through` does not list | add the package to `through`, or use one it lists |
| E207 | a call to a service that is not an open host service | the upstream lists it under `open host service`, or the call goes |
| E108 | an OpenAPI or AsyncAPI document that does not read | fix where it points; YAML beyond what goes to JSON and back (tags, `?` keys) is not read |
| E210 | a document uses a channel or an HTTP operation that is not an open host service | the upstream opens it under `open host service`, or the document stops using it (ask) |
| E401 | an upstream value with no mapping | add the line the note shows, with the value the team decides |
| E406, E407 | one word in two meanings crosses | rename, `as`, or map it in an anticorruption layer (ask) |
| E502 | the linter's settings are not what the map writes | run `ritsu sakai build` again with the same `--lang`, and commit |

Fix the first diagnostic first: an error of words or ownership stops what comes after it.

## 5. The linters and the page

`ritsu sakai build --target import-linter|dependency-cruiser|archunit|go-arch-lint` writes the settings where the map's `code` line puts the language; CI runs the linter and `ritsu sakai build … --check`.
Run dependency-cruiser with `--output-type err`, and with TypeScript below 6 (with TypeScript 6 or later, dependency-cruiser 16 reads nothing and passes).
`targets.md` has the settings and what each tool catches.

`ritsu sakai doc` writes the page for people, Markdown with a Mermaid map or one HTML file, for those who run the business, those who run the systems, and the developers who read the code.
`ritsu sakai export cml` writes the map for Context Mapper.
