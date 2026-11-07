# tickets

A ticket service in Python's standard library ([server.py](server.py)). It gives each ticket a new
uuid on every run, and answers `401` to a call that does not show its token. `POST /tickets` makes a
ticket and answers it, with a `Location`; `GET`, `PUT`, `PATCH` and `DELETE /tickets/<id>` read it,
replace it, change it and take it away. [api.yaml](api.yaml) is the OpenAPI document that says what
it answers. [tickets.geas](tickets.geas) holds three claims, and [tickets.ja.geas](tickets.ja.geas)
is the same claims named in Japanese:

```geas
# Claims for a small ticket service, which numbers what it makes itself and asks for a token.
# `keep` holds a value for the steps below it: the token from geas's own environment, which geas
# never shows or writes, and the number the service gave the ticket, which differs on every run. The
# journal and the baseline write them as {token} and {id}, so two runs write the same bytes.

target api {
  serve "python3 server.py {port}"
  port auto
  contract "api.yaml"
}
…
claim "a ticket can be read back, changed and taken away" {
  keep env "TICKETS_TOKEN" as token
  when api.post("/tickets", body: "{\"title\":\"the printer\"}", header "authorization": "Bearer {token}")
  then status is 201
  keep body json ".id" as id
  and  header "location" is "/tickets/{id}"
  when api.get("/tickets/{id}", header "authorization": "Bearer {token}")
  then body json ".id" is "{id}"
  and  body json ".state" is "open"
…
```

```console
$ export TICKETS_TOKEN=a-token-of-yours
$ geas check examples/tickets/tickets.geas
ok 1 - a ticket can be read back, changed and taken away
ok 2 - the address of a new ticket can be followed
ok 3 - a call without the token is refused
3 claims · 3 ok · 0 failed · journal: examples/tickets/.geas/tickets.journal.jsonl
$ geas snap examples/tickets/tickets.geas
3 claims · 3 ok · 0 failed · baseline: 9 interactions → examples/tickets/.geas/tickets.baseline.jsonl
$ geas drift examples/tickets/tickets.geas
drift: 9 interactions compared · 0 drifted · 0 unclaimed change(s) · 0 claimed
```

It needs python3, and the variable: the service reads the token it asks for from `TICKETS_TOKEN` too.
Without it, the claims that read it end in E038, and geas says which variable.

What to look for in the claims file:

- **The token is in no file.** `keep env` reads it from geas's environment, and the journal, the
  baseline, the report and `--json` write it `{token}`. It is the one kind of `keep` that can come before
  a `when`.
- **The number is the service's.** The claims never write one: `keep body json ".id" as id` holds it,
  and `{id}` is every path after it. The journal writes it `{id}` too, so the drift above compares new
  numbers and says nothing has changed.
- **The second claim keeps `.id` and does not use it.** A value no `keep` holds stays what it is in the
  records, and differs from run to run. A claim keeps or masks what it does not use.
- **A whole `Location` can be the next call.** `keep header "location" as at` and `get("{at}", …)`: an
  address on this machine goes as its path.
- **The third claim shows no token.** The `401` is in the document, so that answer is held as well.
- **All five calls are held to the document** by the one `contract` line. A `{id}` in a path fits
  `/tickets/{id}` there as the characters it is.

[tests/changes/tickets/after/server.py](../../tests/changes/tickets/after/server.py) is the service
after an agent renamed `id` to `ticketId`. Copied over `server.py`, it leaves the claims nothing to go on
from. Each stops at its `keep`, and the steps below it do not run:

```text
not ok 1 - a ticket can be read back, changed and taken away
    examples/tickets/tickets.geas:17: contract POST /tickets — got $: the required id is missing
        17 |   when api.post("/tickets", body: "{\"title\":\"the printer\"}", header "authorization": "Bearer {token}")
    examples/tickets/tickets.geas:19: keep body json ".id" as id — got <no key `id` in JSON object>
        19 |   keep body json ".id" as id
      the run that gets there:
          16  keep env "TICKETS_TOKEN" as token  →  {token}
          17  when api.post("/tickets", body: "{"title":"the printer"}", header "authorization": "Bearer {token}")  →  201, body "{\"ticketId\": \"<uuid>\", \"state\": \"open\", \"title\":…
…
ok 3 - a call without the token is refused
3 claims · 1 ok · 2 failed · journal: examples/tickets/.geas/tickets.journal.jsonl
$ echo $?
1
```

The top-level [README](../../README.md#going-on-from-what-a-service-answers) tells the same story.
