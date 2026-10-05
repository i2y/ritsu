# openspec_greeter

What geas's example greeter answers, read from an OpenSpec spec of the service
(`openspec/specs/greeting/spec.md`, written for this example and checked with OpenSpec 1.14.0's
`openspec validate`). Each of the spec's three requirements is pinned by the hash of its block and
read by one requirement of `greeter.req`, met by `server.py` and checked by geas's claims, which are
named as the spec's scenarios are.

`openspec/changes/trim-names/` is a change not yet archived: it modifies `Greeting by name` (names
are trimmed, and a name of spaces is refused) and adds `Health check`. `yuen source outdated` says
what it does to the requirements pinned; `diffs/propose.diff` is the pull request that adds the
change's folder, and `diffs/archive.diff` what `openspec archive` does to the spec, both for
`yuen affected`.

`greeter.ja.req` is the same project in Japanese, reading the spec written in Japanese under `ja/`
(its claims are those of `greeter.ja.geas`). Each `.req` is a project of its own: give `yuen check`
one file at a time.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on
2026-10-05:

- `greeter.geas`, `greeter.ja.geas`, `server.py`: geas, `examples/greeter/`
- `.geas/greeter.map.jsonl`, `.geas/greeter.ja.map.jsonl`: yuen, `examples/greeter/.geas/` (what
  `geas map` wrote for the same claims and the same server)

The pins were written by `yuen source pin`, and `reviewed/` and the `reviewed` lines by
`yuen review --date 2026-10-05`.

```console
$ ritsu yuen check examples/openspec_greeter/greeter.req --root examples/openspec_greeter
$ ritsu yuen check examples/openspec_greeter/greeter.ja.req --root examples/openspec_greeter
$ ritsu yuen source outdated examples/openspec_greeter/greeter.req --root examples/openspec_greeter
$ ritsu yuen affected examples/openspec_greeter/greeter.req --root examples/openspec_greeter --diff examples/openspec_greeter/diffs/propose.diff
```
