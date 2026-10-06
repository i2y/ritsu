# sakai

A small language for the part of a context map that can be checked: which bounded context owns each artifact, which references the relationships let cross a boundary, and how an anticorruption layer maps the values and words that cross.
sakai is one of the eight languages of [ritsu](../../README.md), and reads the artifacts of the others (rules, workflows, calendars, books, `.proto` files) through ritsu, and the contracts services keep in OpenAPI and AsyncAPI documents.

A context map usually lives in a drawing, and the drawing stops matching the code the week after it is drawn.
sakai writes the map next to the artifacts, one file per context, owned by that context's team, and holds every artifact to it:

```ctx
context Delivery(delivery) v1
description "Decides the day to ship, chooses how to carry, and arranges delivery of the parcel"
owner "Delivery team"

owns
  dir "../delivery", "../proto/shop/delivery"
…
upstream Inventory anticorruption layer
  through warehouse.v1
  layer dir "../py/delivery/acl/inventory", "../ts/delivery/acl/inventory"
  layer dir "../java/src/main/java/delivery/acl/inventory", "../go/delivery/acl/inventory"
  enum PackingStatus -> shipping_decision
    PACKING_STATUS_WAITING -> wait
    PACKING_STATUS_PACKED  -> ship
    PACKING_STATUS_SHORT   -> refuse "A box with an item missing is not shipped. It goes back to ordering"

downstream Billing supplier

shared kernel with Billing
  koyomi "../calendars/tokyo_business_days.cal"
```

```console
$ sakai check examples/shop/shop.ctx
examples/shop/shop.ctx: ok — 5 contexts, 7 relationships; 79 artifacts, each in one context; 9 crossings checked (proto 1, rulec 2, koyomi 1, dandori 5)
```

The nine crossings are what the artifacts themselves say: a `.proto` importing another, a rule's `import proto` and `shape`, a calendar's `use calendar`, a workflow's `use rule`, `use proto`, `connect` and child flow.
sakai reads them through ritsu's ports, in the same process, from each language's own reading of its files.

## What the checker says

When Inventory adds a value to `PackingStatus`, protobuf calls it a compatible change and nothing on the wire stops it.
Delivery's anticorruption layer has no line for it, and the check stops until someone decides what the new value becomes:

```text
error[E401]: contexts/delivery.ctx:33:3: The anticorruption layer of Delivery maps no value for PACKING_STATUS_DAMAGED of Inventory's enum warehouse.v1.PackingStatus
    33 |   enum PackingStatus -> shipping_decision
  = PACKING_STATUS_DAMAGED is the value at proto/warehouse/v1/stock.proto:43.
  = Every value of the upstream enum gets a value of the downstream or refuse; when the upstream adds a value, the check fails until someone decides what it becomes.
  = The line, fixed: PACKING_STATUS_DAMAGED -> refuse "…"
```

Two contexts may use one word in two meanings; that is what a boundary is for.
It goes wrong when the word crosses the boundary: here Ordering writes its own `reservation`, and Inventory's `reservation` reaches it through the `.proto` it imports.

```text
error[E406]: contexts/ordering.ctx:22:3: Inventory's reservation crosses into Ordering, where reservation means something else, unmapped
    22 |   reservation "Giving a line of a customer's order the day it will be delivered"
  = Inventory's reservation is "Holding shelf stock for one line of an order until it ships or is cancelled", and means proto "proto/warehouse/v1/stock.proto" message ReserveResponse.
  = Ordering's reservation is "Giving a line of a customer's order the day it will be delivered".
  = Ordering is a conformist of Inventory, so it has no mapping to map it with.
```

sakai never decides that two definitions mean the same; only `as` says so.
The other checks: every artifact covered belongs to exactly one context; a reference crosses only where a relationship allows it (a conformist reads the upstream's published language, an anticorruption layer only from inside its layer, a service is called only when it is an open host service); the patterns agree (a customer needs its supplier to agree, a shared kernel lists the same files on both sides); and separate ways means nothing crosses.
Every code is in [docs/codes.md](docs/codes.md), with when it comes, how to fix it, and the smallest reproduction (`sakai explain E401`).

## Contracts between services: OpenAPI and AsyncAPI

Services that talk over HTTP and events write what they say in contracts: an OpenAPI document for an HTTP API, an AsyncAPI document for the channels a service sends to and receives from.
sakai holds them as artifacts, as it holds a `.proto`: a context publishes its documents as a published language, and opens the HTTP operations and the channels anyone may use.

```ctx
published language payments.v1
  openapi "../payments/api/payments.yaml"
  asyncapi "../payments/events/payments.yaml"
  open host service createCharge, getCharge, paymentSucceeded, paymentFailed
```

What crosses a boundary is what the documents write: a `$ref` to another context's document, and an operation that sends to or receives from another context's channel.
Each is held to the relationships as any reference is, and an anticorruption layer maps an enum of a document value by value, as it maps a `.proto`'s:

```ctx
upstream Payments anticorruption layer
  through payments.v1
  layer dir "../shipping/acl"
  enum ChargeStatus -> enum ShipmentGate
    pending   -> hold
    succeeded -> release
    failed    -> refuse "An order whose charge failed is not shipped"
    refunded  -> refuse "A refunded order is not shipped"
```

```console
$ sakai check examples/webshop/webshop.ctx
examples/webshop/webshop.ctx: ok — 4 contexts, 5 relationships; 9 artifacts, each in one context; 7 crossings checked (openapi 1, asyncapi 5, dandori 1)
```

When Payments stops opening the channel Shipping receives from:

```text
error[E210]: shipping/acl/payments.yaml:11:5: The document shipping/acl/payments.yaml of Shipping uses the channel paymentFailed of Payments (receive), which is no open host service of Payments
    11 |     $ref: '../../payments/events/payments.yaml#/channels/paymentFailed'
  = The open host services of the published language payments.v1 are createCharge, getCharge, paymentSucceeded.
  = Across a boundary, a document sends to and receives from only the channels the other side lists under `open host service` (and the same for HTTP operations): have it listed there, or use what the other side opens.
  involved:
      Shipping  shipping/acl/payments.yaml:11                                   $ref: ../../payments/events/payments.yaml#/channels/paymentFailed
      Payments  asyncapi "payments/events/payments.yaml" channel paymentFailed  a part of the published language payments.v1
```

sakai reads OpenAPI 3.0, 3.1 and 3.2, and AsyncAPI 3.0 and 3.1, written in JSON or in YAML.
Of YAML it reads what goes to JSON and back (RFC 9512, section 3.4, as OpenAPI 3.2 and AsyncAPI 3.1 ask), and stops with E108 at the rest (a tag, a key written with `?`, a second document) rather than read part of it.
A call no contract writes (HTTP to a URL held in a string, a queue with no document) is still not checked, and the page of `doc` says so; nor does sakai check that the code calls what its contract says.
[examples/webshop](examples/webshop/README.md) is a map of four services that talk this way.

## Keys, plain connections and authentication

sakai has three of ritsu's checks of security (the 9xx codes, which mean the same in every language of ritsu).
Each is a warning, and goes away when the file says the thing is meant:

| Code | What it finds | How a file says it is meant |
|---|---|---|
| W901 | a key of a known shape (AWS, GitHub, Slack, Stripe, OpenAI, Anthropic, Google, a PEM private key) written in the map or a context file | `ritsu: test secret` in a comment on the same line |
| W902 | a server of a document of the map that does not encrypt the connection to a host off this machine: `http://` or `ws://` in OpenAPI, `http`, `ws`, `amqp`, `mqtt`, `stomp` or `kafka` in AsyncAPI | `x-ritsu-plaintext: "<why>"` in the server |
| W903 | an operation of a published language's OpenAPI document with no `security` (nor the document), or an AsyncAPI channel on a server with no `security` and no operation that has it | `security: []` on the operation, the document or the server |

The diagnostic of a key gives its kind, prefix and length, never the key, and a line another diagnostic shows has its keys masked.
The keys of the documents and the `.proto` files are said once, by `ritsu check`, since other languages read them too.
Which operation is allowed to whom is written in sekisho's gates (`.gate`), not here.
sakai holds each gate, and each file of Cedar written by hand that a context names (`cedar "…"` under `owns`), to the map like any artifact: an operation is guarded only by a gate, or the Cedar, of the context that holds its contract (E211).
With the `security` of Payments' API taken away:

```text
warning[W903]: payments/api/payments.yaml:9:7: The operation createCharge of the published language payments.v1 says no authentication
     9 |       operationId: createCharge
  = Neither the operation nor the document has `security`, so a reader of the contract cannot tell how a client proves who it is.
  = Add `security` to the operation, or to the whole document. If the operation is open to anyone on purpose, write `security: []` on it.
  involved:
      Payments  openapi "payments/api/payments.yaml" operation createCharge  POST /charges
```

## The page of the map

`sakai doc` writes a page for the people who have to understand what the code is to do and check it: those who run the business, those who run the systems, and the developers who read the code.
It has the context map, each context's artifacts with what their languages say they hold (a rule's inputs and outputs, a book's accounts and transfers, a calendar's data), its published language and glossary, each relationship with the references that cross it, and the mappings of the anticorruption layers, including the one a rule's `import proto` decides.
The Markdown draws the map with Mermaid, which GitHub shows; the HTML is one file that reads nothing from outside, and a box of its map goes to the context.
[tests/golden/doc/shop.en.md](tests/golden/doc/shop.en.md) is the example's page.

## The imports of the code

sakai does not read imports: each language has a linter that teams already run in CI, and `sakai build` writes its settings from the map.

```console
$ sakai build examples/shop/shop.ctx --target import-linter --check
examples/shop/py/.importlinter: Up to date (11 contracts)
$ sakai build examples/shop/shop.ctx --target go-arch-lint --check
examples/shop/go/.go-arch-lint.yml: Up to date (11 components)
```

- Python: import-linter. A namespace package (a directory without `__init__.py`, as protoc writes) under a package is not read by import-linter, so sakai names it as a root of its own.
- TypeScript and JavaScript: dependency-cruiser. Run it with `--output-type err` in CI; `json` exits 0 even with violations. Mind TypeScript's version: dependency-cruiser 16 reads TypeScript below 6, and with TypeScript 6 or later it reads no `.ts` file and passes without a word. The tests run TypeScript 5.9.3.
- Java: ArchUnit. It reads what compiled classes use, so it catches a class used by its full name with no import, and does not see an import nothing uses; the other three read import statements.
- Go: go-arch-lint.
- Rust: no linter. `sakai check` asks Cargo for the crates and holds their dependencies to the map.

Run `--check` in CI with the `--lang` the settings were written with: the settings explain each rule in that language.
[docs/targets.md](docs/targets.md) has the settings, what each tool catches, and how the map is written as Context Mapper's CML (`sakai export cml`).

## For AI agents

[skills/sakai](../../skills/sakai/SKILL.md) is a skill for an agent that writes or fixes a map: the flow, the language on one page, what to ask a person, and how to fix each diagnostic.
Its pages besides `SKILL.md` are copies of `docs/`, made by `skills/sync.sh`.

## Install

sakai comes with ritsu:

```console
$ cargo install --git https://github.com/i2y/ritsu --locked ritsu
```

Run it as `ritsu sakai …`.
Called through a link named `sakai`, ritsu is sakai, with every language sakai reads joined; the archives of a release carry the link, and `ln -s ritsu sakai` beside the binary makes one.
The binary of sakai's own crate joins no other language: it stops with E104 at a map that holds rules, calendars or workflows.

## Commands

```console
$ sakai check examples/shop/shop.ctx
$ sakai build examples/shop/shop.ctx --target dependency-cruiser
$ sakai export cml examples/shop/shop.ctx
$ sakai doc examples/shop/shop.ctx --format html --out site
$ sakai api examples/shop/shop.ctx
$ sakai explain E401
```

`check` takes map files and directories, and `--format json`; `--root` changes the directory paths are counted from (else the nearest one above that holds `.git`).
Every command takes `--lang ja|en` (else `SAKAI_LANG`, then `RITSU_LANG`, else English) and `--help`.
The exit code is 0 for no errors, 1 for errors, and 2 for bad arguments, a file that cannot be read, or a language that is not joined.

## Examples

- [examples/shop](examples/shop/README.md): a small shop in five contexts and seven relationships, with every pattern, over real workflows, rules, calendars, a book and `.proto` files, and code in Python, TypeScript, Java and Go.
- [examples/shop.ja](examples/shop.ja/README.ja.md): the same shop with Japanese names.
- [examples/webshop](examples/webshop/README.md): four services of a web shop that talk over HTTP and events, by their OpenAPI and AsyncAPI documents, with an anticorruption layer that maps an enum of a document; [examples/webshop.ja](examples/webshop.ja/README.ja.md) is the same with Japanese names.
- [ritsu.ctx](../../ritsu.ctx) and [contexts/](../../contexts) at the root of ritsu: ritsu's own crates, in thirteen contexts, their dependencies held to the map by `ritsu check ritsu.ctx` in CI.

## How it is checked

`cargo test -p sakai` runs the language on its fixtures and on the example, and runs the real tools.
On one run of `cargo test -p sakai -- --nocapture` on macOS on Apple silicon, with every tool there and no test skipped: 205 tests, 28 seconds once built.

- 170 mutants, each a fixture or an example with one change, are held to the diagnostics they give, in English and in Japanese (83 of them with Japanese names, each with an English twin, and 4 for Rust).
- The four import linters run on 56 copies (14 for each tool): the example and a map whose generated code sits inside a context, each as it is and with imports added that the map forbids, with English and Japanese names. Every tool passes the copies as they are and catches each added import.
- Context Mapper 6.12.0's validator, with every check, finds nothing in the CML of the examples.
- ritsu's reader of YAML is held to the YAML test suite (its release data-2022-01-17, 402 cases): it reads 204 as the suite's JSON, does not read 104 that go beyond what goes to JSON and back, does not read any of the 94 that are no YAML, and reads none of them as another value.
- buf compares sakai's reading of every `.proto` of the example and the fixtures.
- The pages of `doc` are golden files; Mermaid 11 and 12 draw every chart, and Chrome opens the HTML page, where a box of the map goes to its context.
- The outputs, diagnostics, settings and `.ctx` lines in this README, `docs/` and the skill are real: `tests/docs.rs` runs and compares them.

## Status

The language, `check`, `build`, `export cml`, `doc`, `api` and `explain` are done, as in [DESIGN.md](DESIGN.md).
Not yet: checking owners against CODEOWNERS, a rule's `import jsonschema` as the target of a mapping, generating the code of a mapping, and run-time calls that no contract writes as artifacts (DESIGN 14, 15.11).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
The copy of the Cabinet Office's table of national holidays in the examples keeps its own terms ([THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
