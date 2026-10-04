# greeter

What geas's example greeter answers, one requirement for each of its claims (greets by name, refuses an empty name, keeps a running total, answers 404 to any other path). What the service answers was decided for this example, so each requirement says where it comes from with `decided`. Each is met by `file "server.py"` and checked by a claim of `greeter.geas`; the scope is `server.py`, which traces to the requirements through geas's record of the lines each claim runs.

`greeter.req` is in English, and `greeter.ja.req` is the same project in Japanese (its claims are those of `greeter.ja.geas`). Each `.req` is a project of its own: give `yuen check` one file at a time.

`.geas/greeter.map.jsonl` and `.geas/greeter.ja.map.jsonl` are the records `geas map <spec> --root .` wrote here. `changes/change.diff` is a change to `server.py` (the text of the answer to an empty name), and `changes/after.map.jsonl` the record after it, for `yuen affected`.

Copied from the other languages of ritsu (`crates/<language>/` in this repository), as they were on 2026-10-04:

- `greeter.geas`: geas, `examples/greeter/greeter.geas`
- `greeter.ja.geas`: geas, `examples/greeter/greeter.ja.geas`
- `server.py`: geas, `examples/greeter/server.py`

`reviewed/` and the `reviewed` and `approved` lines were written by `yuen review --date 2026-10-04`.

```console
$ ritsu yuen check examples/greeter/greeter.req
$ ritsu yuen check examples/greeter/greeter.ja.req
```
