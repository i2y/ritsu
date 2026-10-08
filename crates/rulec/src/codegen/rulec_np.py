"""rulec's vectorised evaluator: the rule is data, and this file is the whole interpreter.

Nothing here is generated. `rulec gen` writes `<alias>.json` — one rule's tables,
definitions and result lowered to integers — and this module turns it into a closure over
numpy arrays once, when the plan is loaded. Every call after that is column arithmetic.

What makes that possible is what rulec refuses. A cell tests its own column and nothing
else, so one cell is one comparison over a whole column, a row is the elementwise
conjunction of its cells, and a table is a single `np.select`.

Two of the proofs `rulec check` runs are load-bearing here rather than decorative:

  * E105 proved that the rows of a `unique` table do not overlap, so first-match and
    unique-match are the same function on this data and no runtime uniqueness test is
    needed. E101 proved the rows are exhaustive, so `np.select` needs no default.
  * E108 proved every intermediate fits in int64 — which is what makes it safe to compute
    in numpy's int64, where overflow wraps silently instead of raising.

A rule that walks a sequence (`fold`) is not written for this target: a walk carries state
from element to element, which is not a column operation. `rulec gen` names it and skips it.
"""

from __future__ import annotations

import datetime
import json

import numpy as np

_EPOCH = datetime.date(1970, 1, 1)


_NOVALUE = object()


class RuleInputError(ValueError):
    """An input outside what the rule declares. The same refusal the generated code makes.

    The sentence, the value and the row travel apart, so a caller can react to
    which row was refused without parsing the text back. The sentence is the one the
    other languages raise for the same input, in the language the plan was written in.
    """

    def __init__(self, what: str, value: object = _NOVALUE, row: int | None = None, word: str = "row") -> None:
        super().__init__(what)
        self.what = what
        self.value = value
        self.row = row
        self.word = word

    def __str__(self) -> str:
        if self.value is _NOVALUE:
            m = self.what
        else:
            # A number prints as itself and anything else as its repr, so a string keeps its
            # quotes; a numpy scalar first becomes the Python value it holds (its repr says
            # `np.int64(5)` and `np.True_`).
            v = self.value.item() if isinstance(self.value, np.generic) else self.value
            num = isinstance(v, (int, float)) and not isinstance(v, bool)
            m = f"{self.what}: {v}" if num else f"{self.what}: {v!r}"
        return m if self.row is None else f"{m} ({self.word} {self.row})"


class RuleContradictionError(AssertionError):
    """Two rows of a `unique` table matched one element: the rule contradicted itself.

    The guard W114 leaves where the checker could not decide a pair. It is never the
    caller's fault, and it names the element as the input error does.
    """

    def __init__(self, what: str, row: int | None = None, word: str = "row") -> None:
        super().__init__(what)
        self.what = what
        self.row = row
        self.word = word

    def __str__(self) -> str:
        return self.what if self.row is None else f"{self.what} ({self.word} {self.row})"


def _not_whole(v):
    """The elements of a column of numbers that are not whole numbers, as a mask.

    Every generated door refuses a number that is not an integer before it looks at the
    range: a float sits inside any range, and 18.3 for a rate in steps of 0.1% would be
    taken as 1.83%. A whole float is the integer it equals, as SQL's door reads it, and a
    boolean is no number at all.
    """
    if v.dtype == bool:
        return np.ones(len(v), dtype=bool)
    if np.issubdtype(v.dtype, np.integer):
        return np.zeros(len(v), dtype=bool)
    if np.issubdtype(v.dtype, np.floating):
        with np.errstate(invalid="ignore"):
            return ~np.isfinite(v) | (v != np.floor(v))

    def bad(x):
        if isinstance(x, (bool, np.bool_)):
            return True
        if isinstance(x, (int, np.integer)):
            return False
        if isinstance(x, (float, np.floating)):
            return not (np.isfinite(x) and float(x).is_integer())
        return True

    return np.array([bad(x) for x in v], dtype=bool)


def _column(col):
    """A column as a one-dimensional array of what it holds.

    `np.asarray` makes a list of lists two-dimensional, and the element that was a list was then
    no element at all: a column with `[]` in it lost its length, and nothing refused it. A value
    that is a list is no value of any input, and the checks below refuse it as one.
    """
    try:
        v = np.asarray(col)
    except (ValueError, TypeError):
        # A list and a number in one column is no array numpy will make.
        return _objects(col)
    return v if v.ndim == 1 else _objects(col)


def _objects(xs):
    """A one-dimensional array of objects, whatever they are: `np.array(xs, dtype=object)` makes
    a list of lists two-dimensional too."""
    out = np.empty(len(xs), dtype=object)
    for i, x in enumerate(xs):
        out[i] = x
    return out


def _not_bool(col):
    """The elements of a column of truth values that are not `True` or `False`, as a mask.

    Every generated door takes JSON's true and false and nothing else: `astype(bool)` read
    the string "false" and the number 1 as true. The column is read as it came, because
    `np.asarray` makes strings of a list that mixes a truth value with a string.
    """
    if isinstance(col, np.ndarray) and col.dtype == bool:
        return np.zeros(len(col), dtype=bool)
    return np.array([not isinstance(x, (bool, np.bool_)) for x in col], dtype=bool)


def _ord(s: str) -> int:
    y, m, d = (int(x) for x in s.split("-"))
    return (datetime.date(y, m, d) - _EPOCH).days


def _ord_or_none(x):
    """A date on the wire as its day number, or None when it is not a date: a YYYY-MM-DD
    string of a day the calendar has, as every door reads one."""
    if not isinstance(x, str) or len(x) != 10 or x[4] != "-" or x[7] != "-":
        return None
    if not all(c in "0123456789" for c in x[:4] + x[5:7] + x[8:]):
        return None
    try:
        return _ord(x)
    except ValueError:
        return None


# --- The five roundings, one array at a time. Every one of them is the generated
# helper of the same name with `np.where` where that has an `if`, so the direction on a
# negative value is pinned the same way.

def _split(x, g):
    a = np.abs(x)
    return np.divmod(a, g)


def _signed(x, v):
    return np.where(x < 0, -v, v)


def _round_down(x, g):
    q, _ = _split(x, g)
    return _signed(x, q * g)


def _round_up(x, g):
    q, r = _split(x, g)
    return _signed(x, np.where(r != 0, (q + 1) * g, q * g))


def _round_half(x, g):
    q, r = _split(x, g)
    return _signed(x, np.where(2 * r >= g, (q + 1) * g, q * g))


def _round_half_down(x, g):
    q, r = _split(x, g)
    return _signed(x, np.where(2 * r > g, (q + 1) * g, q * g))


def _round_bankers(x, g):
    q, r = _split(x, g)
    up = (2 * r > g) | ((2 * r == g) & (q % 2 == 1))
    return _signed(x, np.where(up, (q + 1) * g, q * g))


_ROUND = {
    "down": _round_down,
    "up": _round_up,
    "half_up": _round_half,
    "half_down": _round_half_down,
    "half_even": _round_bankers,
}


def _ev(e, env):
    """One expression node. Values are the stored integers, already at the plan's scale."""
    k = e["k"]
    if k == "name":
        return env[e["n"]]
    if k == "int":
        return np.int64(e["v"])
    if k == "bool":
        return np.bool_(e["v"])
    if k == "bin":
        a, b = _ev(e["l"], env), _ev(e["r"], env)
        op = e["op"]
        if op == "+":
            return a + b
        if op == "-":
            return a - b
        if op == "*":
            return a * b
        if op == "//":
            return a // b
        if op == "<=":
            return a <= b
        if op == "<":
            return a < b
        if op == ">=":
            return a >= b
        if op == ">":
            return a > b
        if op == "==":
            return a == b
        raise ValueError(f"unknown operator: {op}")
    if k == "fn":
        a = [_ev(x, env) for x in e["a"]]
        if e["f"] == "min":
            return np.minimum(a[0], a[1])
        if e["f"] == "max":
            return np.maximum(a[0], a[1])
        raise ValueError(f"unknown function: {e['f']}")
    if k == "round":
        return _ROUND[e["mode"]](_ev(e["x"], env), _ev(e["g"], env))
    raise ValueError(f"unknown node: {k}")


def _val(v, env):
    """A cell on the output side: a literal, or a name read at the column's scale."""
    return _ev(v, env) if isinstance(v, dict) else v


def _unreachable(kind):
    return 0 if kind in ("int", "date") else False if kind == "bool" else ""


def _test(t, v, absent=None):
    """One cell, over a whole column. `None` would be a `-`, which the plan leaves out.

    `absent` is where an optional number, date or truth value has no value: its column holds a
    placeholder there, which no test but `none` and `not:` may take."""
    op = t["op"]
    if op == "none":
        return absent if absent is not None else np.zeros(len(v), dtype=bool)
    if op == "true":
        c = v
    elif op == "false":
        c = ~v
    elif op == "eq":
        c = v == t["v"]
    elif op == "in":
        c = np.isin(v, t["vals"])
    elif op == "notin":
        c = ~np.isin(v, t["vals"])
    elif op == "cmp":
        c = None
        for o, lit in t["tests"]:
            one = v <= lit if o == "<=" else v >= lit if o == ">=" else v < lit if o == "<" else v > lit
            c = one if c is None else (c & one)
    else:
        raise ValueError(f"unknown cell: {op}")
    if absent is None:
        return c
    # `not:` holds of the absent value; every other test is about a value that is there.
    return c | absent if op == "notin" else c & ~absent


class Rule:
    """One `.rule`, loaded. Call it with whole columns; it answers with whole columns."""

    def __init__(self, plan: dict):
        self.plan = plan
        self.name = plan["rule"]
        self.alias = plan["alias"]
        self.version = plan["version"]
        self.sha256 = plan["sha256"]
        self.inputs = [i["name"] for i in plan["inputs"]]
        self.outputs = [o["name"] for o in plan["outputs"]]
        # The machine this rule is one step of, when it is one: the carried
        # column, where a case starts, and the states it ends in.
        self.machine = plan.get("machine")

    def __repr__(self) -> str:
        return f"<rulec.Rule {self.name} v{self.version} {self.sha256[:12]}>"

    def _columns(self, cols: dict):
        """The entry guard, and the wire turned into integers. Refusing is part of the rule.

        It refuses what every other generated door refuses, in the same order and with the
        same sentences: each column's type, enum and range, then each `constraint` between
        two columns, then the days of a koyomi date. Each refusal names the first element
        it is about.
        """
        env, n = {}, None
        word = self.plan.get("row", "row")

        def refuse(what, value=_NOVALUE, row=None):
            return RuleInputError(what, value, row, word)

        for spec in self.plan["inputs"]:
            name = spec["name"]
            say = spec.get("say", {})
            if name not in cols:
                raise refuse(say.get("missing", f"{name} is missing"))
            v = _column(cols[name])
            n = len(v) if n is None else n
            if len(v) != n:
                raise refuse(f"{say.get('length', name)} ({len(v)} ≠ {n})")
            wire = v
            # The absent value of an optional column. It arrives as JSON null (`None` here,
            # or a NaN once numpy has widened the column), and the plan tests it as the word
            # "none" — `astype(str)` alone turned it into "None" and the guard refused it.
            if spec.get("optional"):
                v = _objects(["none" if x is None or (isinstance(x, float) and x != x) else x for x in v])
            kind = spec["kind"]
            # `null` in a column that is not optional is an input that is not there.
            if not spec.get("optional"):
                bad = np.array([x is None for x in cols[name]], dtype=bool)
                if bad.any():
                    raise refuse(say.get("missing", f"{name} is missing"), None, int(np.argmax(bad)))
            # The absent value of an optional column is the word itself, not a wrong type.
            absent = np.array([x == "none" for x in v], dtype=bool) if spec.get("optional") else np.zeros(n, dtype=bool)
            # A number, a date or a truth value holds a placeholder where it is absent, and the
            # cells read where it is absent from beside it: a column of numbers has no word in
            # it to test (§15.201).
            fill = lambda x, z: _objects([z if a else e for e, a in zip(x, absent)])
            if kind == "date":
                days = [0 if a else _ord_or_none(x) for x, a in zip(v, absent)]
                bad = np.array([d is None for d in days], dtype=bool) & ~absent
                if bad.any():
                    i = int(np.argmax(bad))
                    raise refuse(say["date"], wire[i], i)
                v = np.array(days, dtype=np.int64)
            elif kind == "int":
                bad = _not_whole(v) & ~absent
                if bad.any():
                    i = int(np.argmax(bad))
                    raise refuse(say["integer"], wire[i], i)
                v = (fill(v, 0) if spec.get("optional") else v).astype(np.int64)
            elif kind == "bool":
                bad = _not_bool(cols[name]) & ~absent
                if bad.any():
                    i = int(np.argmax(bad))
                    raise refuse(say["boolean"], cols[name][i], i)
                v = (fill(v, False) if spec.get("optional") else v).astype(bool)
            else:
                bad = np.array([not isinstance(x, str) for x in cols[name]], dtype=bool) & ~absent
                if kind == "str" and bad.any():
                    i = int(np.argmax(bad))
                    raise refuse(say["string"], cols[name][i], i)
                v = v.astype(str)
            if spec.get("optional") and kind in ("date", "int", "bool"):
                env[("absent", name)] = absent
            if "values" in spec:
                bad = ~np.isin(v, spec["values"])
                if bad.any():
                    i = int(np.argmax(bad))
                    raise refuse(say["enum"], v[i], i)
            lo, hi = spec.get("min"), spec.get("max")
            if lo is not None or hi is not None:
                bad = np.zeros(n, dtype=bool)
                if lo is not None:
                    bad |= v < lo
                if hi is not None:
                    bad |= v > hi
                # The range is the range of the values that are there (§15.201).
                bad &= ~absent
                if bad.any():
                    i = int(np.argmax(bad))
                    raise refuse(say["range"], wire[i], i)
            env[name] = v
        n = 0 if n is None else n
        # A combination the caller said does not happen: no row was demanded for it, so it is
        # refused rather than answered. Both sides are brought to the step they share first.
        for k in self.plan.get("constraints", []):
            a, b = env[k["left"]] * k["left_times"], env[k["right"]] * k["right_times"]
            op = k["op"]
            holds = a <= b if op == "<=" else a < b if op == "<" else a >= b if op == ">=" else a > b
            bad = ~np.asarray(holds, dtype=bool)
            if bad.any():
                raise refuse(k["what"], row=int(np.argmax(bad)))
        # A date of koyomi's days takes those days only.
        for d in self.plan.get("days", []):
            v = env[d["name"]]
            on = np.zeros(n, dtype=bool)
            for lo, hi in d["runs"]:
                on |= (v >= lo) & (v <= hi)
            if (~on).any():
                raise refuse(d["what"], row=int(np.argmax(~on)))
        return env, n

    def traced(self, **cols):
        """The outputs and, per table, the 1-based row that decided each element."""
        env, n = self._columns(cols)
        fired = []
        for st in self.plan["steps"]:
            if st["op"] == "define":
                env[st["name"]] = _ev(st["expr"], env)
                continue
            conds, picks = [], []
            for row in st["rows"]:
                c = np.ones(n, dtype=bool)
                for t in row["tests"]:
                    c = c & _test(t, env[t["col"]], env.get(("absent", t["col"])))
                conds.append(c)
                picks.append(row["vals"])
            # The default is never selected: E101 proved the rows cover the declared space
            # and the entry guard above is what holds the input inside it. It is written all
            # the same, because `np.select` has to be told the dtype of a column of words.
            # W114: a pair of rows the checker could not prove apart. Where both match one
            # element, the rule contradicts itself, and the element is not answered.
            for g in st.get("guards", []):
                both = conds[g["a"]] & conds[g["b"]]
                if both.any():
                    raise RuleContradictionError(g["what"], int(np.argmax(both)), self.plan.get("row", "row"))
            for oi, out in enumerate(st["outs"]):
                env[out] = np.select(
                    conds, [_val(p[oi], env) for p in picks], default=_unreachable(st["kinds"][oi])
                )
            # Which row decided each element. A merged set's rows come from different tables,
            # so the trace entry travels with the row rather than with the step.
            fired.append((np.select(conds, np.arange(len(conds)), default=0), [r["fired"] for r in st["rows"]]))
        out = {}
        for spec in self.plan["outputs"]:
            e = spec.get("expr")
            raw = _ev(e, env) if e is not None else env[spec["name"]]
            r = spec.get("round")
            if r is not None:
                raw = _ROUND[r["mode"]](raw, r["grid"])
                if r["div"] != 1:
                    raw = raw // r["div"]
            out[spec["name"]] = raw
        return out, fired

    def __call__(self, **cols):
        return self.traced(**cols)[0]


def load(path: str) -> Rule:
    """Read a plan `rulec gen` wrote. The closure is built here, not per call."""
    with open(path, encoding="utf-8") as f:
        return Rule(json.load(f))
