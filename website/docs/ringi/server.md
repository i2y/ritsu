# What is generated, and what the server does

*For the engineer who builds or runs the server requests go through.*

ritsu writes the code of the requests and stops there. The code is a library of pure functions,
with no clock, no storage and no network: given a ticket, an event, the organization and the time,
it answers the next ticket and what the server should do. Keeping the tickets, running the clocks,
sending the notices and serving the screens is the server's. What ritsu writes for it is a contract:
the functions, the documents that describe them, and the vectors a server is tested with.
[ringi-server](https://github.com/i2y/ringi-server), a framework in Go, keeps this contract, and
[a server on it](#a-server-on-ringi-server) takes a few commands.

## In each language's package

`ritsu gen --target typescript` (and `python`, `go`, `rust`) writes, for each request type of the
project, the code of the rules and dates it was lowered to, the module that joins them to the
ticket, and once for the project the ticket machine itself. Beside them is the list of the request
types (`ticket_kinds.ts`, `ticket_kinds.py`, `ticketkinds/ticketkinds.go`, `ticket_kinds.rs`):

| Entry | Takes | Answers |
|---|---|---|
| `start` | the request as submitted (the ticket's ID, the requester, the form), the organization, the time | the new ticket and what to do, or why the request is refused |
| `next` | the ticket as it was, the event, the organization, the time | the same |
| `worked` | a form filled in part | which fields are shown and required now, and the values of the computed ones |

Each entry takes JSON and answers JSON, in one shape for every kind of request, so a server runs
every kind from this list without code of its own for any. What it is to do comes back as a list,
each with a key no other has: notify these people, start or cancel this clock, hold, post or let go
this amount in this book, start this flow.

## In `<out>/requests/`

`ritsu gen --target requests` writes, once for the project and in no language of code, what
describes the requests to a server, a screen and an agent. The screens name the catalog of
components they are drawn with by a URL, which `--catalog` gives:

```console
$ cd crates/ringi/examples
$ ritsu gen leave.ringi --target requests --catalog https://example.com/ritsu/catalog.json --out generated
generated: generated/requests/types/leave.json
generated: generated/requests/openapi.json
generated: generated/requests/mcp/tools.json
generated: generated/requests/organization/schema.json
generated: generated/requests/organization/columns.md
generated: generated/requests/organization/find.jsonl
generated: generated/requests/vectors/leave.jsonl
generated: generated/requests/vectors/worked/leave.jsonl
generated: generated/requests/a2ui/leave/form.en.json
generated: generated/requests/a2ui/leave/form.ja.json
generated: generated/requests/a2ui/leave/card.en.json
generated: generated/requests/a2ui/leave/card.ja.json
generated: generated/requests/a2ui/leave/list.en.json
generated: generated/requests/a2ui/leave/list.ja.json
generated: generated/requests/a2ui/leave/mine.en.json
generated: generated/requests/a2ui/leave/mine.ja.json
generated: generated/requests/a2ui/leave/history.en.json
generated: generated/requests/a2ui/leave/history.ja.json
generated: generated/requests/a2ui/leave/offered.en.json
generated: generated/requests/a2ui/leave/offered.ja.json
generated: generated/requests/a2ui/catalog.json
generated: generated/requests/a2ui/functions.js
```

| File | What it is |
|---|---|
| `types/<name>.json` | each request type: its form, its stages and who answers them, what people may do, the flows it uses |
| `openapi.json` | the server's API, in OpenAPI 3.1: submitting each kind, the lists of what waits for someone, approving, sending back, withdrawing, and the rest |
| `mcp/tools.json` | the same operations as the tools of an agent, so an assistant can fill a form from the papers it is given, or answer what waits for its user |
| `a2ui/<name>/` | six screens of each kind (the form, the card an approver answers, the list of what waits, the requester's own list, the history, and the notice of the day a request may be made from), in English and in Japanese, as A2UI 1.0 templates, with their catalog and its functions (`a2ui/catalog.json`, `a2ui/functions.js`) |
| `organization/schema.json`, `organization/columns.md` | what the organization's data looks like, as JSON and as the columns of its CSV files |
| `vectors/<name>.jsonl`, `vectors/worked/<name>.jsonl`, `organization/find.jsonl` | the vectors |

The form's checks and computations run in three places: in the generated code, in the screen's
functions, and in the reference interpreter. All three are written from the same lowered rules.

## The vectors

`ritsu ringi vectors` writes, for each request type, every row of its scenarios and of the cases it
makes up from the file (the edges of each quorum and range, the limit of the send-backs, every
person of the organization as the requester, the proxies, a ticket across two versions of the
organization) as one line of JSON: the ticket before, the event, the organization, the time, and
what the reference interpreter answers. They are the files under `vectors/` above. A server is held
to the contract by giving each line to its own code and comparing the answers.

The code ritsu writes in each of the four languages answers every vector of the examples as the
reference interpreter does, refusals included.

## What the server does

The server keeps each ticket and its history, takes the events (a person's operation, a clock, a
flow's answer), and gives them to `next` one at a time per ticket. Before it does, it asks the gate
whether the person may. After it, it saves the ticket, then carries out what it was told to do,
each once, by its key. It hands `next` the part of the organization the ticket needs, cut from the
version in effect, and runs the clocks of the deadlines and the reminders.

How people sign in, where the tickets are kept, how notices go out, where attachments are put,
where the organization's data comes from, and how flows are run, differ from one company to
another, and are the server's to choose.

## A server on ringi-server

[ringi-server](https://github.com/i2y/ringi-server) is a framework in Go that does all of this. It
keeps the tickets and the attachments in PostgreSQL, signs people in through an OpenID Connect
provider, and runs the clocks. It serves every kind of request with the same code: the pages people
use in a browser, with JavaScript or without, the API of `openapi.json`, the A2UI screens, and the
tools of `mcp/tools.json`. A module beside it runs the flows on Temporal. Your project gives it what
is your own: the gate, the notices, the organization's data, the books and the flows.

`ritsu ringi init` writes the first files of a project on it: one request type with the organization,
the calendar and the gate it reads, the code generated from them, a server, its tests and a README.
It takes Go 1.26 or later to run:

```sh
ritsu ringi init requests --module example.com/requests
cd requests
go mod tidy
go run ./cmd/server -dev-login
```

Open `http://127.0.0.1:8080/pages/` and choose who you are. `-dev-login` checks no one, so the server
answers only your own computer. Write your request types in `project/`, beside the first one, then run
`ritsu check project` and `go generate ./...`.

[ringi-server's site](https://i2y.github.io/ringi-server/) shows the pages and says what your project
gives the server, and
[its guide for people who run a server](https://i2y.github.io/ringi-server/operations/) has the
settings, the backups and how to raise its version.

## Seeing it without a server

`ritsu ringi run` is the reference interpreter on a scenario ([ringi](index.md) shows one), and
`ritsu ringi api` prints as JSON what a request type declares: its form with the types and ranges,
its stages, the operations of its ticket, and the days it can be used on.
