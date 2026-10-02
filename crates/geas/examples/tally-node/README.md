# tally-node

A tally kept in memory and served over HTTP, written in TypeScript ([server.ts](server.ts)) and run
by Node, which strips the types (Node 23 or later; no build step). [tally.geas](tally.geas) holds
five claims: it starts at zero, adds what it is given, refuses what is not a number, resets, and
answers 404 to unknown paths. Node's `http` answers with chunked bodies, which geas reads as the
body they carry, and `mask header "date"` keeps the date out of drift.

```console
$ geas check examples/tally-node/tally.geas
$ geas map examples/tally-node/tally.geas --root examples/tally-node
```

`map` needs nothing of the program: geas sets `NODE_V8_COVERAGE` and preloads a small hook through
`NODE_OPTIONS`, so the service writes its coverage when geas stops it with SIGTERM; `map` then
prints `map: 1 file · 38 lines of code, 38 run by some claim`. The lines are the `.ts` file's own,
since Node blanks the types instead of moving the code.

[tests/changes/tally-node](../../tests/changes/tally-node) holds an agent's change, and
[tests/golden/en/languages/tally-node](../../tests/golden/en/languages/tally-node) what the tests
expect of `map` and `geas affected` on it. The answer to the `/health` route it adds is code no
claim runs; the route's own `} else if` line counts as run by the claims that add a number, since
Node decides a line by its first character, the `}` that closes their block.
