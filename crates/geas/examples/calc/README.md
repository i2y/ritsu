# calc

A command-line calculator in Python ([calc.py](calc.py)) and the claims a person reads about it
([calc.geas](calc.geas)). It is the smallest kind of target: `run "python3 calc.py"` starts the
command once for each `when`, with the call's arguments after its words, and the checks read what
it printed and how it exited.

```console
$ geas check examples/calc/calc.geas
```

Every claim holds: four `ok` lines and `4 claims · 4 ok · 0 failed`. The claims use the three
things a command observes: `stdout is "5"`, `exit is 1`, `stderr contains "division by zero"`.

```console
$ geas map examples/calc/calc.geas --root examples/calc
```

`map` runs the same claims with Python's coverage switched on (Python 3.12 or later) and writes
`.geas/calc.map.jsonl`, the lines each claim ran: `map: 1 file · 16 lines of code, 14 run by some
claim`. The two left are lines 8 and 15: `calc.py` also subtracts, and divides when the divisor is
not zero, and no claim says either, so a change there is one `geas affected` calls unclaimed.
[tests/changes/calc](../../tests/changes/calc) holds an agent's change to `calc.py`, and the outputs
the tests expect of `map` and `affected` on it are in
[tests/golden/en/languages/calc](../../tests/golden/en/languages/calc).
