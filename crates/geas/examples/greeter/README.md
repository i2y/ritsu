# greeter

A small HTTP service in Python ([server.py](server.py)): it greets a name, refuses an empty one,
and keeps a running total. [greeter.geas](greeter.geas) holds the claims about it, and
[greeter.ja.geas](greeter.ja.geas) the same claims named and explained in Japanese, for a person
who reads them in Japanese (`--lang ja` gives the messages in Japanese too).

The target is a service, `serve "python3 server.py {port}"` with `port auto`: each claim starts its
own server on a free port, so "totals accumulate across requests" builds its total from zero, and
the claims can run side by side (`--jobs 4`). The two `mask` lines say that the `date` and `server`
headers change by nature, so drift does not compare them.

```console
$ geas check examples/greeter/greeter.geas
$ geas check examples/greeter/greeter.ja.geas --lang ja
```

[server_refactored.py](server_refactored.py) is the same service after an agent cleaned it up.
Every claim still holds, and drift finds seven changes no claim promises:

```console
$ geas snap examples/greeter/greeter.geas
$ cp examples/greeter/server_refactored.py examples/greeter/server.py
$ geas drift examples/greeter/greeter.geas
$ git checkout examples/greeter/server.py
```

The top-level [README](../../README.md) shows each of these runs, the claim that catches a service
without the check for an empty name, and `geas map` and `geas affected` on an agent's change to
`server.py` ([tests/changes/greeter](../../tests/changes/greeter)).
