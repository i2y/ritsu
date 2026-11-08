# webhooks

A service in Python's standard library ([server.py](server.py)) that keeps the state of one
subscription from the events a payment provider posts to `/events`. Each event carries the number of
its place in the order things happened (`seq`) and the status it leaves the subscription in; `GET
/subscription` answers the state. The provider can deliver its events in another order, and can
deliver one twice, so the handler keeps the number of the last event it applied and drops an event
that is not newer. [webhooks.geas](webhooks.geas) holds two claims, and
[webhooks.ja.geas](webhooks.ja.geas) the same claims named and explained in Japanese:

```geas
target api {
  serve "python3 server.py {port}"
  port auto
}
…
claim "the subscription ends where the latest event says, in any order" {
  in any order {
    when api.post("/events", body: "{\"id\":\"ev_1\",\"seq\":1,\"status\":\"past_due\"}")
    then status is 200
    when api.post("/events", body: "{\"id\":\"ev_2\",\"seq\":2,\"status\":\"active\"}")
    then status is 200
    when api.post("/events", body: "{\"id\":\"ev_3\",\"seq\":3,\"status\":\"canceled\"}")
    then status is 200
  }
  when api.get("/subscription")
  then body json ".status" is "canceled"
}
…
```

```console
$ geas check examples/webhooks/webhooks.geas
ok 1 - the subscription ends where the latest event says, in any order (6 orders)
ok 2 - an event delivered twice changes nothing (12 orders)
2 claims · 2 ok · 0 failed · journal: examples/webhooks/.geas/webhooks.journal.jsonl
```

It needs python3. geas runs each claim once for each order of the steps in its block, each on a new
server: the three events of the first claim in 6 orders, and the four of the second, one of them
written twice, in 12. [server_as_it_comes.py](server_as_it_comes.py) is the same service with a
handler that writes each event as it comes. It holds when the events come in the order they
happened, and both claims fail in the orders that end on an older event:

```console
$ cp examples/webhooks/server_as_it_comes.py examples/webhooks/server.py
$ geas check examples/webhooks/webhooks.geas
not ok 1 - the subscription ends where the latest event says, in any order
    examples/webhooks/webhooks.geas:12: in any order — 4 of 6 orders fail; below is the first, order 2, and the others are orders 4-6
…
not ok 2 - an event delivered twice changes nothing
    examples/webhooks/webhooks.geas:27: in any order — 9 of 12 orders fail; below is the first, order 2, and the others are orders 4-6 and 8-12
…
2 claims · 0 ok · 2 failed · journal: examples/webhooks/.geas/webhooks.journal.jsonl
$ echo $?
1
```

What to look for in the claims file:

- **A step moves with what is under it.** The `then status is 200` under each `when` goes with it
  into every order; the `when` after the block runs last in every order and reads the state the
  events left.
- **A delivery that comes twice is written twice.** Steps written the same are not told apart, so
  the second claim has 4!/2! orders. The claim says everything it sends, as any claim does.
- **The orders are fixed.** The written order comes first, then the dictionary order of where the
  steps are written, so the report, the journal and the baseline are the same bytes in every run.
- **What it does not try.** geas makes one call at a time; a handler that goes wrong only when two
  events are handled at the same time is not caught here.
