# orders

An order service in Python's standard library ([server.py](server.py)). `POST /orders` with a whole
number makes an order of that total and answers it, with a `Location`; `GET /orders/<id>` answers the
order, or `404`. [api.yaml](api.yaml) is the OpenAPI document that says what it answers, and
[orders.geas](orders.geas) holds three claims and one `contract` line:

```geas
target api {
  serve "python3 server.py {port}"
  port auto
  contract "api.yaml"
}

claim "a new order is open" {
  when api.post("/orders", body: "2500")
  then status is 201
  when api.get("/orders/1")
  then status is 200
  and  body json ".state" is "open"
}
…
```

```console
$ geas check examples/orders/orders.geas
ok 1 - a new order is open
ok 2 - a total that is not a number is refused
ok 3 - an unknown order is not found
```

It needs python3. Each `get` and `post` is one more check, on its `when`'s line: the status is one
the document lists, the `Location` it requires is there, the media type is one it lists, and the
body fits the schema. The journal writes each with `"check":"contract"`.

What to look for in the claims file:

- **The claims do not read the order.** They read `.state` and the status. The total, the date and
  the `Location` are what the document holds.
- **The refusals are in the document too.** The `400` and the `404` are listed there with their own
  body, so those answers are held as well. A status the document does not list fails the claim, even
  when the claim checks for it.
- **The claims choose the calls.** An answer no claim brings about is not held. geas does not call
  anything because the document lists it.

[tests/changes/orders/after/server.py](../../tests/changes/orders/after/server.py) is the service
after an agent made the total a string and renamed `placed` to `placedAt`. Copied over `server.py`, it
fails the first claim, though every check the claim writes still holds:

```text
not ok 1 - a new order is open
    examples/orders/orders.geas:13: contract POST /orders — got $: the required placed is missing; $.total: "2500" is string; the contract says integer
        13 |   when api.post("/orders", body: "2500")
    examples/orders/orders.geas:15: contract GET /orders/{id} — got $: the required placed is missing; $.total: "2500" is string; the contract says integer
        15 |   when api.get("/orders/1")
      the run that gets there:
          13  when api.post("/orders", body: "2500")  →  201, body "{\"id\": \"1\", \"state\": \"open\", \"total\": \"2500\", \"placedAt\": \"2026-11-07\"}"
          15  when api.get("/orders/1")  →  200, body "{\"id\": \"1\", \"state\": \"open\", \"total\": \"2500\", \"placedAt\": \"2026-11-07\"}"
```

A claim that goes on after an answer breaks the document: the second `when` still runs, and says what
it found too. The top-level [README](../../README.md#holding-what-a-service-answers-to-its-contract)
shows the same run.
