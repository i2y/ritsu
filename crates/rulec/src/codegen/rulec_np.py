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


class RuleInputError(ValueError):
    """An input outside what the rule declares. The same refusal the generated code makes."""


def _ord(s: str) -> int:
    y, m, d = (int(x) for x in s.split("-"))
    return (datetime.date(y, m, d) - _EPOCH).days


# --- The five roundings (§7.3), one array at a time. Every one of them is the generated
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


def _test(t, v):
    """One cell, over a whole column. `None` would be a `-`, which the plan leaves out."""
    op = t["op"]
    if op == "true":
        return v
    if op == "false":
        return ~v
    if op == "eq":
        return v == t["v"]
    if op == "in":
        return np.isin(v, t["vals"])
    if op == "notin":
        return ~np.isin(v, t["vals"])
    if op == "cmp":
        c = None
        for o, lit in t["tests"]:
            one = v <= lit if o == "<=" else v >= lit if o == ">=" else v < lit if o == "<" else v > lit
            c = one if c is None else (c & one)
        return c
    raise ValueError(f"unknown cell: {op}")


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

    def __repr__(self) -> str:
        return f"<rulec.Rule {self.name} v{self.version} {self.sha256[:12]}>"

    def _columns(self, cols: dict):
        """The entry guard, and the wire turned into integers. Refusing is part of the rule."""
        env, n = {}, None
        for spec in self.plan["inputs"]:
            name = spec["name"]
            if name not in cols:
                raise RuleInputError(f"{name}: 渡されていません")
            v = np.asarray(cols[name])
            n = len(v) if n is None else n
            if len(v) != n:
                raise RuleInputError(f"{name}: 列の長さが揃っていません ({len(v)} ≠ {n})")
            kind = spec["kind"]
            if kind == "date":
                v = np.array([_ord(str(x)) for x in v], dtype=np.int64)
            elif kind == "int":
                v = v.astype(np.int64)
            elif kind == "bool":
                v = v.astype(bool)
            else:
                v = v.astype(str)
            if "values" in spec:
                bad = ~np.isin(v, spec["values"])
                if bad.any():
                    i = int(np.argmax(bad))
                    raise RuleInputError(f"{name}: 列挙 {spec['enum']} の値ではありません: {v[i]!r} (行 {i})")
            lo, hi = spec.get("min"), spec.get("max")
            if lo is not None or hi is not None:
                bad = np.zeros(n, dtype=bool)
                if lo is not None:
                    bad |= v < lo
                if hi is not None:
                    bad |= v > hi
                if bad.any():
                    i = int(np.argmax(bad))
                    raise RuleInputError(f"{name}: 範囲の外です: {v[i]} (行 {i})")
            env[name] = v
        return env, (0 if n is None else n)

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
                    c = c & _test(t, env[t["col"]])
                conds.append(c)
                picks.append(row["vals"])
            # The default is never selected: E101 proved the rows cover the declared space
            # and the entry guard above is what holds the input inside it. It is written all
            # the same, because `np.select` has to be told the dtype of a column of words.
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
