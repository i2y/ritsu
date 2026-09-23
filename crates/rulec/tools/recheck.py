#!/usr/bin/env python3
"""Re-check a rulec certificate, without rulec.

`rulec certificate <file.rule>` prints the evidence behind two of the five things `check`
proves. This program holds it to the claims, and shares no code with the tool that wrote
it — that is the whole point. It reads one certificate on stdin, or the files named as
arguments, and says for each table what it re-checked.

    $ rulec certificate rules/送料.rule | python3 tools/recheck.py
    送料料金: unique, 8 rows — 28 pairs disjoint, 8 rows reached                       ok

What it checks, per rule:

  * **the units** (§2.1): every value's type is derived here from the leaves up — a name's
    type is the one the rule declares, a literal's is what its unit says — and has to come
    out as the type the rule declares for the value. Units that do not meet stop here.
  * **int64** (§7.4): every named value's interval is recomputed here, by interval
    arithmetic over the declared ranges and the expression the certificate carries, and the
    integer it stores at its scale has to fit in a signed 64-bit word.

What it checks, per table:

  * every pair of rows of a `unique` table is either **proved disjoint** — the certificate
    names one axis, and the two rows really do take no coordinate in common there — or
    listed as **undecided**, which the rule's own W114 warning already said out loud. A
    pair that is in neither list is a certificate that does not hold.
  * every row is **reached**: the point the certificate names lies inside that row's box,
    and, under `policy first`, inside no earlier row's box. A row an `apply` brought in and
    this rule never uses is listed as unused instead — the rule does not claim to reach it.
  * the table is **complete**: the cover is a tree whose every internal node has one child
    per coordinate of the axis at its depth, so it tiles the space by shape; every leaf
    either names a row that takes the whole subtree, or says why no input reaches it — a
    `constraint` that cannot hold there, or a derived value whose coordinate lies outside
    what the derive can produce. Both of those are re-checked here from the ranges.

Given `--rule <file.rule>` it also checks that the certificate is **about that text**: the
digest matches, and every cell of every row is read back out of the file at the byte span
the certificate names and held beside the parsed form it states. A literal is turned into
a number by looking it up among the axis's own boundary coordinates, so no unit table is
needed here; a literal that is not one of them is counted and reported rather than passed.

A leaf that rests on the tables above is re-checked too, and only with what the certificate
carries: the columns each table decides, the value each of its rows writes, and the boxes
those rows take are all here, so the span a value puts an input into is recomputed rather
than believed. Such a leaf has to come with its reason or it is refused.

What it does **not** check: the pairs the axes do not part, which are listed rather than
proved.

Exit code 0 when every table holds, 1 when one does not, 2 on a certificate it cannot
read. No dependencies; Python 3.9 or later.
"""

import hashlib
import json
import math
import re
import sys
from fractions import Fraction


class Bad(Exception):
    """A claim the certificate makes that does not hold."""


class Unreadable(Exception):
    """A leaf this program cannot type — a word whose type only the context gives. The
    value it sits in is reported as not re-checked, which is neither passing it nor
    failing it."""


def numeric(t):
    """Whether a value of this type is stored as an integer, and so has an int64 claim to
    make. A truth value, a date, a string and an enum do not; everything §2.1 writes with a
    unit does, as do `number` and `rate`."""
    return t in ("number", "rate") or "[" in (t or "")


I64_MAX = 2**63 - 1
GROUPS = {}


def num(x):
    """A rational as the certificate writes it: "7/2", "3", or null for an open end."""
    return None if x is None else Fraction(x)


def guarantees(cert):
    """The `a <= b` pairs the `constraint` lines promise, closed under chains: two lines
    saying a running total never passes the next, and that one never passes the whole,
    together say the first never passes the whole. rulec reads them that way, so a
    re-checker that did not would refuse an honest certificate."""
    le = set()
    for k in cert.get("constraints", []) or []:
        l, r, op = k.get("left"), k.get("right"), k.get("op")
        if op in ("<=", "<"):
            le.add((l, r))
        elif op in (">=", ">"):
            le.add((r, l))
    while True:
        more = {(a, d) for a, b in le for c, d in le if b == c}
        if more <= le:
            return le
        le |= more


def interval(e, ranges, le=frozenset()):
    """The interval an expression is forced into by the declared ranges, and by `le` — the
    `a <= b` pairs the caller guarantees, which is what bounds a share (§15.102). None where
    the certificate's own arithmetic gives up too (an unbounded name, a divisor that spans
    zero, a share with no guarantee behind it); the claim is then not re-checked here and
    the caller says so."""
    if "name" in e:
        lo, hi = ranges.get(e["name"], (None, None))
        return None if lo is None or hi is None else (num(lo), num(hi))
    if "num" in e:
        v = num(e.get("value"))
        return None if v is None else (v, v)
    if "op" in e:
        a, b = interval(e["l"], ranges, le), interval(e["r"], ranges, le)
        if a is None or b is None:
            return None
        (al, ah), (bl, bh) = a, b
        if e["op"] == "+":
            return (al + bl, ah + bh)
        if e["op"] == "-":
            return (al - bh, ah - bl)
        if e["op"] in ("*", "/"):
            if e["op"] == "/" and bl <= 0 <= bh:
                return None
            vs = sorted(x * y if e["op"] == "*" else x / y for x in (al, ah) for y in (bl, bh))
            return (vs[0], vs[-1])
        return None
    if "op" in e and e["op"] in CMP_OPS:
        return None  # a truth value is not stored as an integer
    if "lit" in e:
        return None
    if "call" in e:
        args = e.get("args", [])
        inner = interval(args[0], ranges, le) if args else None
        if inner is None:
            return None
        # A share: `floor(T x C / S)` (§15.102). The low end is the smallest amount over
        # the largest whole; the high end is the amount itself, and that rests on a
        # `constraint` saying the running total never passes the whole — the ranges alone
        # would give the product of two of them.
        if e["call"] == "allocate" and len(args) == 3:
            run = interval(args[1], ranges, le)
            whole = interval(args[2], ranges, le)
            if run is None or whole is None or whole[0] <= 0:
                return None
            if inner[0] < 0 or run[0] < 0:
                return None
            if ("name" not in args[1] or "name" not in args[2]
                    or (args[1]["name"], args[2]["name"]) not in le):
                return None
            return (math.floor(inner[0] * run[0] / whole[1]), math.floor(inner[1]))
        if e["call"] in ("min", "max") and len(args) == 2:
            other = interval(args[1], ranges, le)
            if other is None:
                return None
            f = min if e["call"] == "min" else max
            return (f(inner[0], other[0]), f(inner[1], other[1]))
        # A rounding lands on the grid: the low end toward zero, the high end away from it,
        # whichever way the mode itself rounds.
        grid = interval(args[1], ranges, le) if len(args) > 1 else None
        if grid is None or grid[0] != grid[1] or grid[0] == 0:
            return None
        # Down to the grid on the low side and up on the high side — `floor` and `ceil`,
        # not "toward zero" and "away from zero". Rounding -7 to a grid of 5 can give -10,
        # which is below the toward-zero reading of -5, so that reading claimed an interval
        # the value can leave (§15.99).
        g = abs(grid[0])
        return (math.floor(inner[0] / g) * g, math.ceil(inner[1] / g) * g)
    return None


def unify(a, b):
    """Two types meet when they are the same, or when one is money with no tax flag and the
    other is the same currency with one — which is how a bare amount is written (§2.1)."""
    if a == b:
        return a
    for x, y in ((a, b), (b, a)):
        if x.startswith("money[") and y.startswith("money[") and y.startswith(x[:-1] + ","):
            return y
    return None


DIMENSIONED = lambda t: t.startswith("money[") or ("[" in t and not t.startswith("money["))


CMP_OPS = ("<=", "<", ">=", ">", "==")


def type_of(e, types):
    """The type of an expression, derived here by the rules of §2.3 rather than taken from
    the certificate. A leaf states its own type — a name's is declared, a literal's is what
    its unit says — and everything above them is derived, so units that do not meet stop
    here the same way E103 stops the check."""
    if "name" in e:
        t = types.get(e["name"])
        if t is None:
            raise Bad(f"{e['name']}: no type is declared for it")
        return t
    if "num" in e:
        return e.get("type", "?")
    if "lit" in e:
        # A date, a string: it carries its own type. A bare word does not — only the
        # context gives it one — and this program says so rather than guessing.
        t = e.get("type")
        if t is None:
            raise Unreadable(f"a literal with no type: {e['lit']}")
        return t
    if "op" in e:
        a, b = type_of(e["l"], types), type_of(e["r"], types)
        op = e["op"]
        if op in CMP_OPS:
            # A comparison is a claim about units too: `注文金額 >= 3万円` is well typed
            # because both sides are 円.
            if unify(a, b) is None:
                raise Bad(f"`{a}` {op} `{b}`: the units do not meet")
            return "bool"
        if op in ("+", "-"):
            t = unify(a, b)
            if t is None:
                raise Bad(f"`{a}` {op} `{b}`: the units do not meet")
            return t
        if op == "*":
            if DIMENSIONED(a) and DIMENSIONED(b):
                raise Bad(f"`{a}` * `{b}`: two dimensions multiplied, which has no type here")
            return a if DIMENSIONED(a) else (b if DIMENSIONED(b) else ("rate" if "rate" in (a, b) else a))
        if op == "/":
            if unify(a, b) is not None:
                return "number"
            if DIMENSIONED(b):
                raise Bad(f"`{a}` / `{b}`: divided by another dimension")
            return a
        raise Bad(f"an operator this program does not know: {op}")
    if "call" in e:
        args = [type_of(x, types) for x in e.get("args", [])]
        if not args:
            raise Bad(f"{e['call']}: called with nothing")
        if e["call"] in ("min", "max"):
            if len(args) > 1 and unify(args[0], args[1]) is None:
                raise Bad(f"{e['call']}(`{args[0]}`, `{args[1]}`): the units do not meet")
        # The two sides of the ratio have to meet; their common type cancels, and the share
        # keeps the type of the amount being handed out.
        if e["call"] == "allocate":
            if len(args) != 3:
                raise Bad("allocate: it takes an amount, a running total and a whole")
            if unify(args[1], args[2]) is None:
                raise Bad(f"allocate(`{args[1]}`, `{args[2]}`): the two sides of the ratio do not meet")
        return args[0]
    raise Bad("an expression this program cannot read")


def declares_as(want, got):
    """What a declaration may call a value. A rate and a count are both dimensionless and
    share one runtime form, so `達成率 : rate = 合計点 / 50` and `基本点 : number = 税込 /
    100円` are both allowed to name theirs (§2.1). Only a declaration gets that latitude —
    inside an expression the rule above stays strict, so a rate still cannot be added to a
    count."""
    return unify(want, got) is not None or {want, got} == {"rate", "number"}


def check_types(cert):
    """Units (§2.1, E103): every value's type is derived here from the leaves up, and has to
    come out as the type the rule declares for it. Returns (checked, not re-checkable)."""
    types = cert.get("types", {})
    n = unread = 0
    for v in cert.get("values", []):
        try:
            got = type_of(v["expr"], types)
        except Unreadable:
            unread += 1
            continue
        want = v.get("type")
        if want is not None and not declares_as(want, got):
            raise Bad(f"{v['name']}: the expression gives `{got}`, the rule declares `{want}`")
        n += 1
    return n, unread


def check_values(cert):
    """int64: recompute each value's interval and hold the integer it stores to i64."""
    le = guarantees(cert)
    declared = {k: (num(v[0]), num(v[1])) for k, v in cert.get("ranges", {}).items()}
    # What bounds a name: the interval this program computed for it where there is one,
    # and the declared range otherwise. Reading the declared range in preference would let
    # a forged `ranges` entry override the arithmetic.
    computed = {}
    ranges = {}
    checked = none_stated = 0
    for v in cert.get("values", []):
        ranges = {**{k: (None if a is None else str(a), None if b is None else str(b))
                     for k, (a, b) in declared.items()},
                  **{k: (str(a), str(b)) for k, (a, b) in computed.items()}}
        got = interval(v["expr"], ranges, le)
        said = v.get("interval")
        if got is None:
            if said is not None:
                raise Bad(f"{v['name']}: it states an interval this program cannot derive")
            if numeric(v.get("type")):
                raise Bad(f"{v['name']}: it states no interval, and a value of its type is stored as an integer")
            none_stated += 1
            continue
        if said is None:
            raise Bad(f"{v['name']}: it states no interval, and this program derives one")
        d = declared.get(v["name"])
        if d is not None and d[0] is not None and d[1] is not None and (got[0] < d[0] or got[1] > d[1]):
            raise Bad(f"{v['name']}: the range the rule declares is narrower than what its expression reaches")
        computed[v["name"]] = got
        said = (num(said[0]), num(said[1]))
        if got[0] < said[0] or got[1] > said[1]:
            raise Bad(f"{v['name']}: the interval here is [{got[0]}, {got[1]}], wider than the stated [{said[0]}, {said[1]}]")
        stored = max(abs(got[0]), abs(got[1])) * v["scale"]
        if int(stored) > I64_MAX:
            raise Bad(f"{v['name']}: stores up to {int(stored)}, past int64")
        if int(stored) > int(Fraction(v["stored_max"])):
            raise Bad(f"{v['name']}: stores up to {int(stored)}, more than the stated {v['stored_max']}")
        checked += 1
    return checked, none_stated


def bound_at(t, ai, ci):
    b = (t["axes"][ai].get("bounds") or [None] * len(t["axes"][ai]["coords"]))[ci]
    return (None, None) if b is None else (num(b[0]), num(b[1]))


def span_of_name(t, name, path):
    """What the box allows a name: the coordinate where the path has fixed one, and the
    declared range where it has not — the same reading the tool's own sieve uses (§6.2)."""
    col = [a["column"] for a in t["axes"]]
    if name in col and col.index(name) < len(path):
        return bound_at(t, col.index(name), path[col.index(name)])
    r = t.get("_ranges", {}).get(name)
    return (None, None) if r is None else (num(r[0]), num(r[1]))


def left_out(t, name, path):
    """Whether the coordinate the box fixes for a name leaves its ends out: an interval does,
    a single value does not, and a declared range (a name the box fixes nothing for) does
    not either."""
    col = [a["column"] for a in t["axes"]]
    if name in col and col.index(name) < len(path):
        a, b = bound_at(t, col.index(name), path[col.index(name)])
        return not (a is not None and b is not None and a == b)
    return False


def constraint_rules_out(t, k, path):
    """Whether one `constraint` is impossible over the whole of this box. Where the two ends
    meet, the pair is out of reach when one of them is an end its coordinate leaves out
    (§15.140)."""
    (ll, lh) = span_of_name(t, k["left"], path)
    (rl, rh) = span_of_name(t, k["right"], path)
    shut = left_out(t, k["left"], path) or left_out(t, k["right"], path)
    return {
        "<=": ll is not None and rh is not None and (ll > rh or (ll == rh and shut)),
        "<": ll is not None and rh is not None and ll >= rh,
        ">=": lh is not None and rl is not None and (lh < rl or (lh == rl and shut)),
        ">": lh is not None and rl is not None and lh <= rl,
    }[k["op"]]


def derived_rules_out(t, ai, path):
    """Whether a derived column's coordinate lies outside what its own expression can
    produce. `None` when this program has no expression for it."""
    if ai >= len(path) or ai >= len(t["axes"]):
        return None
    reach = t.get("_reach_of", {}).get(t["axes"][ai]["column"])
    if reach is None:
        return None
    lo, hi = bound_at(t, ai, path[ai])
    return (hi is not None and hi < reach[0]) or (lo is not None and lo > reach[1])


def closed_at(u, ai, x):
    """A coordinate as a **closed** interval of true values.

    `bounds` gives a coordinate's ends as they are written, and an interval coordinate does
    not contain them: the values sit on the axis's grid, so `(10, 30)` on a grid of 1 is
    `[11, 29]`. A point coordinate has the same value at both ends and stays as it is.
    """
    a, b = bound_at(u, ai, x)
    if a is not None and b is not None and a == b:
        return (a, b)
    st = num(u["axes"][ai].get("step"))
    if st is None:
        return (a, b)
    return (None if a is None else a + st, None if b is None else b - st)


def _taken_earlier(u, row, ai, x):
    """Whether a single earlier row of `u` takes the whole of `row`'s box at coordinate `x`."""
    for e in u["rows"]:
        if e["row"] >= row["row"]:
            continue
        if all(
            all(
                (y not in row["accepts"][a] or (a == ai and y != x)) or y in e["accepts"][a]
                for y in range(len(u["axes"][a]["coords"]))
            )
            for a in range(len(u["axes"]))
        ):
            return True
    return False


def produced_where(tables, column, value, inp):
    """Where a column another table decides can hold a value, read on one of that table's
    own columns.

    The rows of that table that write the value, and the coordinates of `inp` at which one
    of them can still fire. Under `first` a row's box is not where it fires — an earlier row
    may take all of it, and the catch-all at the bottom has the whole axis for a box while
    firing only on what is left — so a row counts only while no single earlier row takes the
    whole of its box at that coordinate. That reading is loose the safe way: what drops out
    is only what certainly cannot fire, so the span that comes back contains every input on
    which the column really holds the value.

    `("num", (lo, hi))` or `("words", {…})`, or `None` when no table here decides the column,
    it has no axis for `inp`, or a row of it writes something this program cannot read.
    """
    for u in tables:
        decides = u.get("decides") or []
        if column not in decides:
            continue
        oi = decides.index(column)
        ai = next((i for i, a in enumerate(u["axes"]) if a["column"] == inp), None)
        if ai is None:
            return None
        picked = []
        for r in u["rows"]:
            p = r.get("produces")
            if not isinstance(p, list) or oi >= len(p) or p[oi] is None:
                return None
            if p[oi] == value:
                picked.append(r)
        if not picked:
            return None
        first = u["policy"] == "first"
        axis = u["axes"][ai]
        live = [
            x
            for x in range(len(axis["coords"]))
            for r in picked
            if x in r["accepts"][ai] and (not first or not _taken_earlier(u, r, ai, x))
        ]
        live = sorted(set(live))
        if not live:
            return None
        if any(b is not None for b in (axis.get("bounds") or [])):
            lo = hi = None
            open_lo = open_hi = False
            for x in live:
                a, b = closed_at(u, ai, x)
                if a is None:
                    open_lo = True
                elif lo is None or a < lo:
                    lo = a
                if b is None:
                    open_hi = True
                elif hi is None or b > hi:
                    hi = b
            return ("num", (None if open_lo else lo, None if open_hi else hi))
        return ("words", frozenset(axis["coords"][x] for x in live))
    return None


def closed_span(t, ai, ci):
    """One coordinate of one axis, as a span."""
    axis = t["axes"][ai]
    if any(b is not None for b in (axis.get("bounds") or [])):
        return ("num", closed_at(t, ai, ci))
    return ("words", frozenset([axis["coords"][ci]]))


def spans_meet(a, b):
    """Whether two spans of one column have anything in common. Two kinds that do not match
    are not compared: saying they meet is the half that refuses to conclude."""
    if a[0] != b[0]:
        return True
    if a[0] == "num":
        (al, ah), (bl, bh) = a[1], b[1]
        return not (
            (ah is not None and bl is not None and ah < bl) or (bh is not None and al is not None and bh < al)
        )
    return bool(a[1] & b[1])


def span_contains(outer, inner):
    if outer[0] != inner[0]:
        return False
    if outer[0] == "num":
        (ol, oh), (il, ih) = outer[1], inner[1]
        lo_ok = ol is None or (il is not None and ol <= il)
        hi_ok = oh is None or (ih is not None and ih <= oh)
        return lo_ok and hi_ok
    return inner[1] <= outer[1]


def stated_span(st, kind):
    """A span as the certificate writes it: two ends for a number, the values otherwise.
    The kind comes from the span this one is held against, so the two shapes never have to
    be told apart by guessing."""
    if not isinstance(st, list):
        return None
    if kind == "num":
        if len(st) != 2 or not all(x is None or isinstance(x, str) for x in st):
            return None
        return ("num", (num(st[0]), num(st[1])))
    if all(isinstance(x, str) for x in st):
        return ("words", frozenset(st))
    return None


def box_holds(t, path, column, value):
    """Whether this box fixes one of its own columns to that value.

    A reason is about the box it is written under, and nothing else ties the two together:
    without this, a reason earned honestly for one box could be copied onto another where
    the columns hold something else entirely, and every span in it would still recompute.
    """
    ai = next((i for i, x in enumerate(t["axes"]) if x["column"] == column), None)
    return ai is not None and ai < len(path) and t["axes"][ai]["coords"][path[ai]] == value


def never_written(t, u, oi, col, val):
    """No row of the table that decides a column writes a value at all."""
    for r in u["rows"]:
        p = r.get("produces")
        if not isinstance(p, list) or oi >= len(p) or p[oi] is None:
            raise Bad(f"{t['table']}: row {r['row']} of {u['table']} writes something this "
                      f"program cannot read, so «{col} is never {val}» cannot be settled")
        if p[oi] == val:
            raise Bad(f"{t['table']}: row {r['row']} of {u['table']} does write {col} = {val}, "
                      f"so it is not a value the tables above never reach")
    return True


def decider(tables, column):
    """The table that decides a column, and where the column sits among its outputs."""
    for u in tables:
        decides = u.get("decides") or []
        if column in decides:
            return u, decides.index(column)
    return None, None


def check_above(t):
    """Every fact this table rests on, earned back from the tables that decide its columns.

    A `never` fact is settled by reading those rows. An `apart` fact is settled by
    recomputing both spans and finding that they do not meet — and the spans the
    certificate wrote have to **contain** the ones recomputed, so it cannot quietly write a
    narrower span than the truth and call apart two things that meet.
    """
    ab = t.get("above") or {}
    axes, tables = t["axes"], (t.get("_tables") or [])

    def named(q):
        ai, ci = q.get("axis"), q.get("coord")
        if not isinstance(ai, int) or not 0 <= ai < len(axes):
            raise Bad(f"{t['table']}: a fact points at axis {ai}, which does not exist")
        coords = axes[ai]["coords"]
        if not isinstance(ci, int) or not 0 <= ci < len(coords):
            raise Bad(f"{t['table']}: a fact points at coordinate {ci} of {axes[ai]['column']}, "
                      f"which does not exist")
        return ai, axes[ai]["column"], coords[ci]

    for f in ab.get("never", []):
        _, col, val = named(f)
        u, oi = decider(tables, col)
        if u is None:
            raise Bad(f"{t['table']}: a fact says {col} is never {val}, and no table here decides {col}")
        never_written(t, u, oi, col, val)

    for f in ab.get("apart", []):
        inp = f.get("input")
        stated = f.get("spans")
        if not isinstance(inp, str) or not isinstance(stated, list) or len(stated) != 2:
            raise Bad(f"{t['table']}: a fact gives a reason this program cannot read")
        got = []
        for q, st in zip((f.get("a") or {}, f.get("b") or {}), stated):
            ai, col, val = named(q)
            if col == inp:
                real = closed_span(t, ai, q["coord"])
            else:
                real = produced_where(tables, col, val, inp)
            if real is None:
                raise Bad(f"{t['table']}: this program cannot work out where {col} = {val} "
                          f"puts {inp}, so the fact is not settled")
            sp = stated_span(st, real[0])
            if sp is None:
                raise Bad(f"{t['table']}: a span for {inp} is written in no shape this program reads")
            if not span_contains(sp, real):
                raise Bad(f"{t['table']}: the span written for {inp} while {col} = {val} is "
                          f"narrower than the one this certificate's own tables give")
            got.append(real)
        if spans_meet(got[0], got[1]):
            raise Bad(f"{t['table']}: the two spans of {inp} meet, so the pair is not apart")


def above_rules_out(t, path):
    """Whether a fact of this table's own rules the box under `path` out."""
    ab = t.get("above") or {}
    for f in ab.get("never", []):
        ai, ci = f.get("axis"), f.get("coord")
        if isinstance(ai, int) and ai < len(path) and path[ai] == ci:
            return True
    for f in ab.get("apart", []):
        a, b = f.get("a") or {}, f.get("b") or {}
        if (isinstance(a.get("axis"), int) and a["axis"] < len(path) and path[a["axis"]] == a.get("coord")
                and isinstance(b.get("axis"), int) and b["axis"] < len(path) and path[b["axis"]] == b.get("coord")):
            return True
    return False


def point_ruled_out(t, path):
    """Whether the sieve rules this point out: some constraint cannot hold at it, or some
    derived column cannot reach it."""
    for k in t.get("constraints", []):
        if constraint_rules_out(t, k, path):
            return True
    return any(derived_rules_out(t, ai, path) for ai in range(len(path)))


def completions(axes, path):
    """Every point the path opens onto."""
    if len(path) >= len(axes):
        return [path]
    out = []
    for c in range(len(axes[len(path)]["coords"])):
        out.extend(completions(axes, path + [c]))
    return out


# --- The linear model (§15.141) ------------------------------------------------------------
#
# A table's certificate names the facts of its linear model by where each comes from — a
# `derive`'s equation, a declared range, a `constraint` — and this program builds each one
# again from the rule's own values, ranges and constraints. A refutation is then a list of
# those facts and of the ends of a box's coordinates, each with a multiplier, and checking it
# is addition: every name cancels and what is left is false.

def factor(e):
    """A literal with no unit, or a rate, as the number it multiplies by — what `rulec`
    accepts as the constant side of a product. None for anything else."""
    if "num" not in e or e.get("type") not in ("number", "rate"):
        return None
    return num(e.get("value"))


def linearize(e, want, types):
    """An expression as a linear form over names of one type, `({name: coefficient}, k)`, or
    None — exactly the shapes `rulec` reads as linear: names of that type, literals of it, sums
    and differences, and products and quotients by a constant."""
    if "name" in e:
        return ({e["name"]: Fraction(1)}, Fraction(0)) if types.get(e["name"]) == want else None
    if "num" in e:
        v = num(e.get("value"))
        return ({}, v) if v is not None and e.get("type") == want else None
    if e.get("op") in ("+", "-"):
        a, b = linearize(e["l"], want, types), linearize(e["r"], want, types)
        if a is None or b is None:
            return None
        sign = 1 if e["op"] == "+" else -1
        terms = dict(a[0])
        for n, c in b[0].items():
            terms[n] = terms.get(n, Fraction(0)) + sign * c
        return (terms, a[1] + sign * b[1])
    if e.get("op") == "*":
        fa, fb = factor(e["l"]), factor(e["r"])
        if (fa is None) == (fb is None):
            return None
        k, side = (fa, e["r"]) if fa is not None else (fb, e["l"])
        inner = linearize(side, want, types)
        return None if inner is None else ({n: c * k for n, c in inner[0].items()}, inner[1] * k)
    if e.get("op") == "/":
        k = factor(e["r"])
        if k is None or k == 0:
            return None
        inner = linearize(e["l"], want, types)
        return None if inner is None else ({n: c / k for n, c in inner[0].items()}, inner[1] / k)
    return None


def model_facts(t, cert):
    """The facts of a table's linear model, each as `(terms, k, strict)` meaning
    `sum(c * name) + k <= 0` (`< 0` when strict), built from the rule rather than read."""
    lin = t.get("linear") or {"extra": [], "facts": []}
    values = {v["name"]: v for v in cert.get("values", [])}
    types = cert.get("types", {})
    out = []
    for f in lin["facts"]:
        if "derive" in f:
            v = values.get(f["derive"])
            if v is None:
                raise Bad(f"{t['table']}: the model names the equation of {f['derive']}, which states no expression")
            got = linearize(v["expr"], v.get("type"), types)
            if got is None:
                raise Bad(f"{t['table']}: the model takes {f['derive']} as linear, and its expression is not")
            terms = {n: -c for n, c in got[0].items()}
            terms[f["derive"]] = terms.get(f["derive"], Fraction(0)) + 1
            k = -got[1]
            if not f["le"]:
                terms, k = {n: -c for n, c in terms.items()}, -k
            out.append((terms, k, False))
        elif "range" in f:
            r = cert.get("ranges", {}).get(f["range"])
            end = None if r is None else num(r[1] if f["hi"] else r[0])
            if end is None:
                raise Bad(f"{t['table']}: the model names an end of {f['range']}'s range that the rule does not declare")
            out.append(({f["range"]: Fraction(1)}, -end, False) if f["hi"] else ({f["range"]: Fraction(-1)}, end, False))
        elif "constraint" in f:
            ks = cert.get("constraints", [])
            i = f["constraint"]
            if not isinstance(i, int) or not 0 <= i < len(ks):
                raise Bad(f"{t['table']}: the model names constraint {i}, which the rule does not have")
            k = ks[i]
            d = {k["left"]: Fraction(1)}
            d[k["right"]] = d.get(k["right"], Fraction(0)) - 1
            if k["op"] in (">=", ">"):
                d = {n: -c for n, c in d.items()}
            out.append((d, Fraction(0), k["op"] in ("<", ">")))
        else:
            raise Bad(f"{t['table']}: a fact of the model comes from nowhere this program knows")
    return out


def beyond(t, ai, coords, hi, at, strict):
    """Whether every coordinate of the box stays on the right side of an end: at or above
    `at` for a low end, above it where the end is left out — and the same below a high one."""
    for c in coords:
        lo, up = bound_at(t, ai, c)
        point = lo is not None and up is not None and lo == up
        v = up if hi else lo
        if v is None:
            return False
        if point:
            ok = (v < at if hi else v > at) or (v == at and not strict)
        else:
            ok = v <= at if hi else v >= at
        if not ok:
            return False
    return True


def farkas_holds(t, cert, refs, box):
    """Whether the multipliers refute the model inside the box. Returns None when they do,
    or what is wrong."""
    facts = t.setdefault("_facts", model_facts(t, cert))
    total, k, strict = {}, Fraction(0), False
    for ref in refs:
        y = num(ref.get("y"))
        if y is None or y < 0:
            return "a multiplier is not a number at least zero"
        if "fact" in ref:
            i = ref["fact"]
            if not isinstance(i, int) or not 0 <= i < len(facts):
                return f"it names fact {i}, which the model does not have"
            terms, fk, fs = facts[i]
        elif "coord" in ref:
            cr = ref["coord"]
            ai, hi, at, left_out = cr.get("axis"), cr.get("hi"), num(cr.get("at")), bool(cr.get("open"))
            if not isinstance(ai, int) or not 0 <= ai < len(t["axes"]) or ai >= len(box) or at is None:
                return f"it names axis {ai}, which does not exist"
            if not beyond(t, ai, box[ai], hi, at, left_out):
                return f"a coordinate of {t['axes'][ai]['column']} in this box lies beyond the end it names"
            col = t["axes"][ai]["column"]
            terms, fk, fs = ({col: Fraction(1)}, -at, left_out) if hi else ({col: Fraction(-1)}, at, left_out)
        else:
            return "it names an inequality from nowhere this program knows"
        for n, c in terms.items():
            total[n] = total.get(n, Fraction(0)) + y * c
        k += y * fk
        strict = strict or (fs and y > 0)
    if any(c != 0 for c in total.values()):
        return "the names do not cancel"
    if k > 0 or (k == 0 and strict):
        return None
    return "what is left is not false"


def facts_hold(t, cert, values, extra):
    """Whether a point's values satisfy every fact of the model. None when they do, or why not."""
    lin = t.get("linear") or {"extra": [], "facts": []}
    if not lin["facts"]:
        return None
    facts = t.setdefault("_facts", model_facts(t, cert))
    at = {}
    for ai, a in enumerate(t["axes"]):
        if values is not None and ai < len(values) and values[ai] is not None:
            at[a["column"]] = num(values[ai])
    for n, v in zip(lin["extra"], extra or []):
        if v is not None:
            at[n] = num(v)
    for terms, k, strict in facts:
        if any(n not in at for n in terms):
            return "no value is given for a name the model uses"
        s = sum(c * at[n] for n, c in terms.items()) + k
        if not (s < 0 if strict else s <= 0):
            return "the values do not satisfy the rule's linear model"
    return None


def check_cover(t):
    """Completeness: the cover has to tile the space, and every leaf has to hold."""
    axes, rows = t["axes"], {r["row"]: r for r in t["rows"]}
    if t.get("cover") is None:
        raise Bad(f"{t['table']}: no cover is stated, so completeness is not shown")
    seen = {"rows": 0, "constraint": 0, "derived": 0, "above": 0, "points": 0, "model": 0}

    def walk(node, path):
        if "split" in node:
            want = len(axes[len(path)]["coords"])
            if len(node["split"]) != want:
                raise Bad(f"{t['table']}: a split at depth {len(path)} has {len(node['split'])} children, the axis has {want}")
            for c, kid in enumerate(node["split"]):
                walk(kid, path + [c])
            return
        if "row" in node:
            r = node["row"]
            if r not in rows:
                raise Bad(f"{t['table']}: the cover names row {r}, which is not in the table")
            for ai, c in enumerate(path):
                if c not in rows[r]["accepts"][ai]:
                    raise Bad(f"{t['table']}: row {r} is said to cover a box it does not take ({axes[ai]['column']})")
            for ai in range(len(path), len(axes)):
                if len(rows[r]["accepts"][ai]) != len(axes[ai]["coords"]):
                    raise Bad(f"{t['table']}: row {r} is said to cover every {axes[ai]['column']}, and does not")
            seen["rows"] += 1
            return
        if "constraint" in node:
            i = node["constraint"]
            if not isinstance(i, int) or not 0 <= i < len(t["constraints"]):
                raise Bad(f"{t['table']}: a leaf names constraint {i}, which is not one of this table's")
            k = t["constraints"][i]
            if not constraint_rules_out(t, k, path):
                raise Bad(f"{t['table']}: {k['left']} {k['op']} {k['right']} can hold here, so the box is not impossible")
            seen["constraint"] += 1
            return
        if "derived_axis" in node:
            ai = node["derived_axis"]
            if not isinstance(ai, int) or not 0 <= ai < len(axes):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which does not exist")
            if ai >= len(path):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which this box has not fixed")
            out = derived_rules_out(t, ai, path)
            if out is None:
                raise Bad(f"{t['table']}: {axes[ai]['column']} is called out of reach, but no expression for it is in the certificate")
            if not out:
                raise Bad(f"{t['table']}: {axes[ai]['column']} can reach this box, so it is not impossible")
            seen["derived"] += 1
            return
        if "farkas" in node:
            box = [[c] for c in path] + [list(range(len(a["coords"]))) for a in axes[len(path):]]
            why = farkas_holds(t, t["_cert"], node["farkas"], box)
            if why is not None:
                raise Bad(f"{t['table']}: a leaf says the linear model has no solution in its box, and {why}")
            seen["model"] += 1
            return
        if node.get("every_point_ruled_out"):
            # Every point of the box, one at a time (§15.98). The sieve is a question about
            # a whole point, so the box is walked out and each point is asked.
            for p in completions(axes, path):
                if not point_ruled_out(t, p):
                    raise Bad(f"{t['table']}: a point of this box is not ruled out, so the box is not impossible")
            seen["points"] += 1
            return
        if "upstream" in node:
            if not any(a.get("kind") == "upstream" for a in axes):
                raise Bad(f"{t['table']}: a leaf rests on a table above, and no column of "
                          f"this table comes from one")
            if not above_rules_out(t, path):
                raise Bad(f"{t['table']}: a leaf rests on the tables above, and no fact this "
                          f"certificate states about them rules this box out")
            seen["above"] += 1
            return
        raise Bad(f"{t['table']}: a leaf of the cover has no kind this program knows")

    walk(t["cover"], [])
    return seen


def check_axis(t, ai):
    """A numeric axis has to tile: its coordinates run from the declared range's low end to
    its high end, each touching the next or one grid step past it, with nothing between.
    §6.2 builds it that way; without this check a coordinate could be quietly removed and
    the gap under it would never be covered by anything (§15.99)."""
    a = t["axes"][ai]
    bs = a.get("bounds") or []
    if not bs or all(b is None for b in bs):
        return 0
    step = num(a.get("step"))
    if step is None or step <= 0:
        raise Bad(f"{t['table']}: {a['column']} has coordinates and no grid to read them on")
    vals = []
    for c, b in enumerate(bs):
        if b is None:
            raise Bad(f"{t['table']}: {a['column']} mixes coordinates that stand for a "
                      f"number with ones that do not")
        lo, hi = num(b[0]), num(b[1])
        if (lo is None and c > 0) or (hi is None and c < len(bs) - 1):
            raise Bad(f"{t['table']}: {a['column']} has an open end away from its ends")
        if lo is not None and hi is not None and lo > hi:
            raise Bad(f"{t['table']}: {a['column']} has a coordinate that runs backwards")
        vals.append((lo, hi))
    for k in range(len(vals) - 1):
        hi, lo = vals[k][1], vals[k + 1][0]
        if not (lo == hi or lo == hi + step):
            raise Bad(f"{t['table']}: {a['column']} has a gap between {hi} and {lo}")
    if a.get("kind") == "input":
        r = t.get("_ranges", {}).get(a["column"])
        if r is not None:
            lo, hi = num(r[0]), num(r[1])
            if vals[0][0] != lo or vals[-1][1] != hi:
                raise Bad(f"{t['table']}: {a['column']} runs from {vals[0][0]} to "
                          f"{vals[-1][1]}, and the rule declares {lo} to {hi}")
    return 1


def axis_splits(axis, tests):
    """Whether every value the cell compares against falls outside every coordinate of the
    axis. §6.2 compresses a column to the boundaries its cells name, so this holds by
    construction — and it is what makes "the coordinate is taken" the same statement as
    "every value in it satisfies the comparison". It is checked, not assumed."""
    for _, v in tests:
        for b in (axis.get("bounds") or []):
            if b is None:
                continue
            lo, hi = num(b[0]), num(b[1])
            if lo is not None and hi is not None and lo == hi:
                continue
            if not ((hi is not None and hi <= v) or (lo is not None and v <= lo)):
                return False
    return True


def coord_admits(bound, op, value):
    """Whether a whole coordinate satisfies one comparison. A coordinate is a point or an
    open interval between two boundaries, and the boundaries are exactly the values the
    cells compare against — so a comparison is constant over a coordinate."""
    lo, hi = bound
    if lo is not None and hi is not None and lo == hi:
        v = lo
        return {"<=": v <= value, "<": v < value, ">=": v >= value, ">": v > value, "=": v == value}[op]
    if op in ("<=", "<"):
        return hi is not None and hi <= value
    if op in (">=", ">"):
        return lo is not None and lo >= value
    return False


def box_from_cells(t, row, groups):
    """The box the row's own cells describe, recomputed here. A certificate that widens a
    box without changing the cell is caught by this and by nothing else."""
    out = []
    for ai, axis in enumerate(t["axes"]):
        cell = row["tests"][ai]
        coords, bounds = axis["coords"], axis.get("bounds", [None] * len(axis["coords"]))
        kind = cell["cell"]
        if kind == "any":
            out.append(list(range(len(coords))))
        elif kind == "none":
            out.append([0] if coords and coords[0] == "none" else [])
        elif kind in ("is", "not"):
            words = set()
            for w in cell["words"]:
                words.add(w)
                words.update(groups.get(w, []))
            hit = [i for i, c in enumerate(coords) if c in words]
            out.append(hit if kind == "is" else [i for i in range(len(coords)) if i not in set(hit)])
        elif kind == "prefix":
            # §6.2 on a string column: a coordinate is taken when its own prefix extends
            # one the cell names. Every prefix a cell names is a coordinate of the axis
            # (that is how the axis was built), which `axis_covers` holds it to.
            ps = axis.get("prefixes")
            if ps is None:
                return None
            if not all(any(p == w for p in ps if p is not None) for w in cell["words"]):
                raise Bad(f"{t['table']}: {axis['column']} is tested on a prefix it has no coordinate for")
            out.append([i for i, p in enumerate(ps) if p is not None and any(p.startswith(w) for w in cell["words"])])
        elif kind == "cmp":
            tests = [(x["op"], num(x["value"])) for x in cell["tests"]]
            if any(v is None for _, v in tests):
                return None  # a literal whose value the certificate could not resolve
            if not axis_splits(axis, tests):
                raise Bad(
                    f"{axis['column']}: the cell compares against a value that falls inside "
                    f"a coordinate, so the axis does not stand for what the cell says")
            hit = []
            for i in range(len(coords)):
                b = bounds[i]
                if b is None:
                    return None
                if all(coord_admits((num(b[0]), num(b[1])), op, v) for op, v in tests):
                    hit.append(i)
            out.append(hit)
        else:
            return None
    return out


def sieve_admits(t, values, at):
    """Whether the values behind a reach point really can occur: each inside the coordinate
    it stands for, each derived column inside its own reach, and every constraint on two of
    this table's columns satisfied. Returns None when they can, or why not.

    Without this the reachability claim would only say "the row's box is not empty", which
    is not what E102 is about."""
    axes, col = t["axes"], [a["column"] for a in t["axes"]]
    if values is None or len(values) != len(axes):
        return "the certificate states no values behind it"
    vs = [None if v is None else num(v) for v in values]
    for ai, (v, c) in enumerate(zip(vs, at)):
        b = (axes[ai].get("bounds") or [None] * len(axes[ai]["coords"]))[c]
        if b is None:
            continue  # an enum or a flag: the sieve has nothing to say about it
        if v is None:
            return f"no value is given for {axes[ai]['column']}"
        lo, hi = num(b[0]), num(b[1])
        if lo is not None and hi is not None and lo == hi:
            if v != lo:
                return f"{axes[ai]['column']} = {v} is not {lo}"
        else:
            if lo is not None and not v > lo:
                return f"{axes[ai]['column']} = {v} is not above {lo}"
            if hi is not None and not v < hi:
                return f"{axes[ai]['column']} = {v} is not below {hi}"
        reach = t.get("_reach_of", {}).get(axes[ai]["column"])
        if reach is not None and axes[ai]["kind"] == "derived" and not (reach[0] <= v <= reach[1]):
            return f"{axes[ai]['column']} = {v} is outside what its own expression reaches"
    for k in t["constraints"]:
        if k["left"] not in col or k["right"] not in col:
            continue
        x, y = vs[col.index(k["left"])], vs[col.index(k["right"])]
        if x is None or y is None:
            continue
        ok = {"<=": x <= y, "<": x < y, ">=": x >= y, ">": x > y}[k["op"]]
        if not ok:
            return f"{k['left']} {k['op']} {k['right']} does not hold at {x}, {y}"
    return None


def check_table(t):
    """Re-check one table. Returns a one-line summary; raises Bad on a claim that fails."""
    name = t["table"]
    axes = t["axes"]
    rows = {r["row"]: r for r in t["rows"]}
    if not rows:
        raise Bad(f"{name}: no rows")
    tiled = 0
    for ai, a in enumerate(axes):
        if not a.get("coords"):
            raise Bad(f"{name}: {a['column']} has no coordinates, so it stands for nothing")
        tiled += check_axis(t, ai)
    if len(rows) != len(t["rows"]):
        # Everything below looks a row up by its number. Two rows sharing one would leave
        # the pair between them unexamined, because neither is earlier than the other.
        raise Bad(f"{name}: two rows are given the same number")
    for r in t["rows"]:
        if len(r["accepts"]) != len(axes):
            raise Bad(f"{name}: row {r['row']} states {len(r['accepts'])} axes, the table has {len(axes)}")
        for ai, coords in enumerate(r["accepts"]):
            n = len(axes[ai]["coords"])
            if any(not isinstance(c, int) or c < 0 or c >= n for c in coords):
                raise Bad(f"{name}: row {r['row']} names a coordinate axis {ai} does not have")

    # (0) The box each row states is the box its own cells describe.
    from_cells = 0
    for r in t["rows"]:
        if "tests" not in r:
            continue
        got = box_from_cells(t, r, GROUPS)
        if got is None:
            continue
        for ai, coords in enumerate(got):
            if sorted(coords) != sorted(r["accepts"][ai]):
                raise Bad(
                    f"{name}: row {r['row']}'s box on {axes[ai]['column']} is not what its cell "
                    f"`{r['cells'][ai]}` describes"
                )
        from_cells += 1

    # (1) No two rows of a `unique` table meet.
    pairs = model_pairs = 0
    if t["policy"] == "unique":
        told = {}
        for d in t["disjoint"]:
            a, b, ai = d["a"], d["b"], d["axis"]
            if a not in rows or b not in rows:
                raise Bad(f"{name}: rows {a} and {b} are not both in the table")
            if not 0 <= ai < len(axes):
                raise Bad(f"{name}: rows {a} and {b} name axis {ai}, which does not exist")
            shared = set(rows[a]["accepts"][ai]) & set(rows[b]["accepts"][ai])
            if shared:
                raise Bad(
                    f"{name}: rows {a} and {b} are said to part on {axes[ai]['column']}, "
                    f"but both take {sorted(shared)[:3]} there"
                )
            told[(a, b)] = ai
            pairs += 1
        # The pairs the axes do not part and the linear model does: no point the rule is
        # asked about is in both rows (§15.141).
        for d in t.get("refuted", []):
            a, b = d["a"], d["b"]
            if a not in rows or b not in rows:
                raise Bad(f"{name}: rows {a} and {b} are not both in the table")
            box = [sorted(set(rows[a]["accepts"][ai]) & set(rows[b]["accepts"][ai])) for ai in range(len(axes))]
            why = farkas_holds(t, t["_cert"], d["farkas"], box)
            if why is not None:
                raise Bad(f"{name}: rows {a} and {b} are said to part on the linear model, and {why}")
            told[(a, b)] = None
            model_pairs += 1
        undecided = {(u["a"], u["b"]) for u in t["undecided"]}
        numbers = sorted(rows)
        for i, a in enumerate(numbers):
            for b in numbers[i + 1:]:
                if (a, b) not in told and (a, b) not in undecided:
                    raise Bad(f"{name}: rows {a} and {b} are neither proved apart nor listed as undecided")
    elif t["disjoint"] or t.get("refuted"):
        raise Bad(f"{name}: a `first` table cannot claim its rows are disjoint")

    # (2) Every row is reached.
    reached, weak = set(), set()
    for r in t["reach"]:
        row, at = r["row"], r["at"]
        if row not in rows:
            raise Bad(f"{name}: row {row} is not in the table")
        if len(at) != len(axes):
            raise Bad(f"{name}: the point for row {row} names {len(at)} axes, the table has {len(axes)}")
        for ai, c in enumerate(at):
            if c not in rows[row]["accepts"][ai]:
                raise Bad(
                    f"{name}: the point for row {row} sits at {axes[ai]['column']} = "
                    f"{axes[ai]['coords'][c]}, which that row does not take"
                )
        why = sieve_admits(t, r.get("at_values"), at)
        if why is None:
            why = facts_hold(t, t["_cert"], r.get("at_values"), r.get("extra_values"))
        if why is not None:
            # Where the certificate hands over no values, the claim falls back to the
            # weaker reading — "the sieve does not exclude this point" — and says so.
            # Where it hands over values that do not hold, it is wrong and fails.
            if r.get("at_values") is None or "no value is given" in why or "states no values" in why:
                weak.add(row)
            else:
                raise Bad(f"{name}: the point for row {row} is one no input reaches — {why}")
        if t["policy"] == "first":
            for earlier in sorted(n for n in rows if n < row):
                if all(c in rows[earlier]["accepts"][ai] for ai, c in enumerate(at)):
                    raise Bad(f"{name}: the point for row {row} is taken by row {earlier} first")
        reached.add(row)
    unused = set(t.get("unused", []))
    # Rows the sieve rules out entirely. E102 does not sieve, so `check` passes them; the
    # certificate names them and this program repeats the name rather than proving it.
    ruled_out = set(t.get("unreachable", []))
    for row in sorted(ruled_out):
        if row in reached or row not in rows:
            raise Bad(f"{name}: row {row} is called unreachable and reached at the same time")
    for row in sorted(unused):
        if row in reached:
            raise Bad(f"{name}: row {row} is called unused and reached at the same time")
        if row not in rows:
            raise Bad(f"{name}: row {row} is not in the table")
    missing = sorted(set(rows) - reached - unused - ruled_out)
    if missing:
        raise Bad(f"{name}: no point is given for row(s) {missing}")

    # (3) The table is complete. What the tables above rule out is earned back first, so a
    # leaf that rests on it rests on something already checked.
    check_above(t)
    seen = check_cover(t)
    if True:
        cover_note = f", {seen['rows']} boxes covered"
        for k, word in (("constraint", "by a constraint"), ("derived", "out of a derive's reach"),
                        ("above", "for the tables above"), ("points", "point by point"),
                        ("model", "by the linear model")):
            if seen[k]:
                cover_note += f" + {seen[k]} impossible {word}"

    kinds = {a["kind"] for a in axes}
    note = "" if kinds == {"input"} else f" (columns: {', '.join(sorted(kinds))})"
    unused_note = f", {len(unused)} unused" if unused else ""
    if ruled_out:
        unused_note += f", {len(ruled_out)} the sieve rules out (stated, not proved)"
    if weak:
        unused_note += f", {len(weak)} with no values behind the point"
        STATED.append(f"{name}: {len(weak)} points come with no values behind them")
    boxes = f", {from_cells} boxes read back from their cells" if from_cells else ""
    apart = f" + {model_pairs} apart on the linear model" if model_pairs else ""
    return (f"{name}: {t['policy']}, {len(rows)} rows — {pairs} pairs disjoint{apart}, "
            f"{len(reached)} rows reached{unused_note}{cover_note}{boxes}{note}, "
            f"{tiled} axes tiled")


OPS = ("<=", ">=", "<", ">", "≦", "≧", "＜", "＞")
SEPS = (",", "、", "，", "・")


def split_words(text):
    for sep in SEPS[1:]:
        text = text.replace(sep, ",")
    return [w.strip() for w in text.split(",") if w.strip()]


def split_cmp(text):
    """`>=1000円 <20000円` as [(op, literal)], or None where it is not a comparison."""
    out, s = [], text.strip()
    while s:
        op = next((o for o in OPS if s.startswith(o)), None)
        if op is None:
            return None
        s = s[len(op):].lstrip()
        op = {"≦": "<=", "≧": ">=", "＜": "<", "＞": ">"}.get(op, op)
        j = len(s)
        for k in range(1, len(s)):
            if any(s.startswith(o, k) for o in OPS):
                j = k
                break
        lit = s[:j].strip()
        if lit.endswith(" and"):
            lit = lit[:-4].strip()
        if not lit:
            return None
        out.append((op, lit))
        s = s[j:].lstrip()
        if s.startswith("and "):
            s = s[4:].lstrip()
    return out or None


def parse_cell(text):
    """The shape of a cell as the file writes it. `("lit", s)` is a bare literal, which is
    a word on an enum column and an `=` comparison on a numeric one."""
    s = text.strip()
    if s in ("", "-"):
        return ("any",)
    if s == "none":
        return ("nothing",)
    if s.startswith("starts_with"):
        rest = s[len("starts_with"):].lstrip()
        if rest.startswith(":"):
            rest = rest[1:].lstrip()
        ws = []
        for part in rest.split(","):
            part = part.strip()
            if not (len(part) >= 2 and part[0] == '"' and part[-1] == '"'):
                return None
            ws.append(part[1:-1])
        return ("prefix", ws) if ws else None
    if s.startswith("not:"):
        return ("not", split_words(s[4:]))
    if any(s.startswith(o) for o in OPS):
        cs = split_cmp(s)
        return ("cmp", cs) if cs else None
    if any(sep in s for sep in SEPS):
        return ("is", split_words(s))
    return ("lit", s)


MULT = {"万": 10000, "億": 100000000, "兆": 1000000000000}
NUMLIT = re.compile(r"^(-?[0-9][0-9_]*(?:\.[0-9]+)?)(万|億|兆)?(.*)$")


def lit_key(text):
    """A literal as (number, unit) where it is one, so that `1万円` and `10000円` are the
    same literal. §2.1 allows one myriad multiplier and then the unit."""
    m = NUMLIT.match(text.strip())
    if not m:
        return ("t", text.strip())
    n = Fraction(m.group(1).replace("_", ""))
    if m.group(2):
        n *= MULT[m.group(2)]
    return ("n", n, m.group(3).strip())


def boundary_values(axis):
    """Every coordinate that stands for one value, by the text the axis writes it as. A
    cell's literal is a boundary of its own column (§6.2), so this is enough to turn the
    text in the file into the number the certificate claims for it."""
    out = {}
    for co, b in zip(axis["coords"], axis.get("bounds") or []):
        if b and b[0] is not None and b[0] == b[1]:
            out[lit_key(co)] = num(b[0])
    return out


def fields_of(line):
    """The byte ranges a table row's `|` separators cut the line into, each trimmed. The
    text after the last `|` is a comment, not a cell, so it is not one of them."""
    bars = [i for i, b in enumerate(line) if b == ord("|")]
    out = []
    for k in range(len(bars) - 1):
        a, b = bars[k] + 1, bars[k + 1]
        seg = line[a:b]
        out.append((a + len(seg) - len(seg.lstrip()), b - (len(seg) - len(seg.rstrip()))))
    return out


def check_cells(cert, path):
    """The table stated here is the table in the file. Returns (cells read back, literals
    whose number could not be pinned, rows written in an applied rule)."""
    with open(path, "rb") as fh:
        lines = fh.read().split(b"\n")
    read = loose = apart = 0
    for t in cert["tables"]:
        axes = t["axes"]
        # Rows written in one table have a cell in the same columns and in no others, and
        # they are written one under another with no other table's rows between them.
        shape, lines_of, seen_axis, at_line, written = {}, {}, [False] * len(axes), {}, {}
        for r in t["rows"]:
            src = r.get("source")
            o = r.get("origin", "")
            # A table is either written in this file or brought in by an `apply`; its rows
            # cannot differ about that. Without this one row could hide behind `null` and
            # never be read back (§15.99).
            was = written.setdefault(o, src is not None)
            if was != (src is not None):
                raise Bad(f"{t['table']}: some rows of `{o}` are written in this file and "
                          f"some are not, which no one table is")
            if src is None:
                continue
            here = tuple(sp is None for sp in src)
            was = shape.setdefault(o, here)
            if was != here:
                raise Bad(f"{t['table']}: rows of `{o}` disagree about which columns they "
                          f"have a cell in")
            ln = r.get("line", 0)
            if not ln:
                raise Bad(f"{t['table']}: row {r['row']} is written in this file and says "
                          f"no line")
            # A row is one line, and the rows of one table are written in order. Without
            # this a span could be pointed at another row's identical text (§15.99).
            if any(sp["line"] != ln for sp in src if sp is not None):
                raise Bad(f"{t['table']}: row {r['row']} has a cell away from the line it "
                          f"is written on")
            prev = at_line.get(o)
            if prev is not None and ln <= prev:
                raise Bad(f"{t['table']}: row {r['row']} of `{o}` is written at line "
                          f"{ln}, at or above the row before it")
            at_line[o] = ln
            lo, hi = lines_of.get(o, (ln, ln))
            lines_of[o] = (min(lo, ln), max(hi, ln))
            for ai, sp in enumerate(src):
                if sp is not None:
                    seen_axis[ai] = True
        for a, (alo, ahi) in lines_of.items():
            for b, (blo, bhi) in lines_of.items():
                if a < b and alo <= bhi and blo <= ahi:
                    raise Bad(f"{t['table']}: rows of `{a}` and `{b}` are written among "
                              f"each other, which no two tables are")
        if lines_of and not all(seen_axis):
            missing = [axes[i]["column"] for i, v in enumerate(seen_axis) if not v]
            raise Bad(f"{t['table']}: no row is written with a cell in {', '.join(missing)}")
        for row in t.get("unused", []):
            src = next((r.get("source") for r in t["rows"] if r["row"] == row), 0)
            if src is not None:
                raise Bad(f"{t['table']}: row {row} is called unused, and it is written in "
                          f"this file rather than brought in by an `apply`")
        for r in t["rows"]:
            src = r.get("source")
            if src is None:
                # A row an `apply` brought in: it is written in another file, and that
                # file's own certificate is where it is read back.
                if r.get("line"):
                    raise Bad(f"{t['table']}: row {r['row']} names a line in this file and "
                              f"no cells")
                apart += 1
                continue
            if len(src) != len(axes):
                raise Bad(f"{t['table']}: row {r['row']} names {len(src)} cells, the table has {len(axes)} columns")
            here = [sp for sp in src if sp is not None]
            ln = r["line"]
            line = lines[ln - 1] if 0 < ln <= len(lines) else b""
            got = sorted((sp["col"], sp["col"] + sp["len"]) for sp in here)
            if not here and b"|" in line:
                # A row with no cell at all is a `clause` whose `when` names no column. A
                # table row cannot become one by having its cells written out of the
                # certificate: the line it sits on has cells on it.
                raise Bad(f"{t['table']}: row {r['row']} is written on a line with cells "
                          f"on it, and the certificate names none of them")
            for (a, b), (c, _) in zip(got, got[1:]):
                if b > c:
                    raise Bad(f"{t['table']}: row {r['row']} names two cells that overlap")
            bars = fields_of(line)
            if bars and got != bars[:len(got)]:
                raise Bad(f"{t['table']}: row {r['row']}'s cells are not the cells of the "
                          f"line it is written on")
            if bars and len(bars) != len(got) + t.get("outputs", 0):
                raise Bad(f"{t['table']}: row {r['row']} is written with {len(bars)} cells, "
                          f"and the certificate accounts for {len(got)} of them")
            for ai, (sp, st) in enumerate(zip(src, r["tests"])):
                if sp is None:
                    # A column the `when` line of a clause does not mention, or one a merged
                    # member table does not have: there is no cell in the file to read back.
                    if st["cell"] != "any":
                        raise Bad(f"{t['table']}: row {r['row']}, {axes[ai]['column']}: "
                                  f"nothing is written there, and the certificate reads `{st['cell']}`")
                    continue
                if sp["len"] == 0 or not sp["text"]:
                    raise Bad(f"{t['table']}: row {r['row']}, {axes[ai]['column']}: an empty cell")
                got1 = line[sp["col"]:sp["col"] + sp["len"]].decode("utf-8", "replace")
                if got1 != sp["text"]:
                    raise Bad(
                        f"{t['table']}: row {r['row']}, {axes[ai]['column']}: the file says "
                        f"`{got1}` where the certificate quotes `{sp['text']}`")
                shape1 = parse_cell(sp["text"])
                if shape1 is None:
                    raise Bad(f"{t['table']}: row {r['row']}, {axes[ai]['column']}: `{sp['text']}` is not a cell this program can read")
                loose += cell_agrees(t, axes[ai], r["row"], shape1, st)
                read += 1
    return read, loose, apart


def cell_agrees(t, axis, row, shape, st):
    """Hold the parsed cell beside the text it came from. Returns how many literals this
    program could not turn into a number."""
    where = f"{t['table']}: row {row}, {axis['column']}"
    kind = st["cell"]
    if shape[0] == "any":
        if kind != "any":
            raise Bad(f"{where}: the file leaves the cell open, the certificate reads it as `{kind}`")
        return 0
    if shape[0] == "nothing":
        if kind != "none":
            raise Bad(f"{where}: the file says `none`, the certificate reads it as `{kind}`")
        return 0
    if shape[0] in ("is", "not", "prefix"):
        if kind != shape[0] or list(st.get("words", [])) != shape[1]:
            raise Bad(f"{where}: the words in the file are not the ones the certificate states")
        return 0
    if shape[0] == "lit":
        if kind == "is":
            if list(st.get("words", [])) != [shape[1]]:
                raise Bad(f"{where}: the file writes `{shape[1]}`, the certificate reads {st.get('words')}")
            return 0
        if kind == "cmp" and len(st["tests"]) == 1 and st["tests"][0]["op"] == "=":
            return literal_agrees(where, axis, shape[1], st["tests"][0]["value"])
        raise Bad(f"{where}: the file writes one literal, the certificate reads it as `{kind}`")
    # a comparison
    if kind != "cmp":
        raise Bad(f"{where}: the file compares, the certificate reads the cell as `{kind}`")
    if len(shape[1]) != len(st["tests"]):
        raise Bad(f"{where}: the file makes {len(shape[1])} comparisons, the certificate states {len(st['tests'])}")
    loose = 0
    for (op, lit), x in zip(shape[1], st["tests"]):
        if op != x["op"]:
            raise Bad(f"{where}: the file writes `{op}`, the certificate states `{x['op']}`")
        loose += literal_agrees(where, axis, lit, x["value"])
    return loose


def axis_span(axis):
    """The whole stretch of values the axis's coordinates cover."""
    lo = hi = None
    for b in (axis.get("bounds") or []):
        if b is None:
            continue
        for v in (num(b[0]), num(b[1])):
            if v is None:
                continue
            lo = v if lo is None else min(lo, v)
            hi = v if hi is None else max(hi, v)
    return lo, hi


def axis_unit(axis):
    """The unit the axis writes its coordinates in, where they all write the same one."""
    us = {lit_key(c)[2] for c in axis["coords"] if lit_key(c)[0] == "n"}
    return us.pop() if len(us) == 1 else None


def literal_agrees(where, axis, text, value):
    """1 when the literal's number could not be pinned, which this program allows in one
    case only: a unit the axis does not write its own coordinates in (`2kg` against an
    axis of grams), where turning the text into a number would take the lexer's unit
    table and this program deliberately has none."""
    bounds = boundary_values(axis)
    k = lit_key(text)
    if k in bounds:
        if value is None or bounds[k] != num(value):
            raise Bad(f"{where}: the file writes `{text}`, the certificate reads it as {value}")
        return 0
    if k[0] != "n" or k[2] != axis_unit(axis):
        return 1
    # Every value a cell compares against inside the declared range is a boundary of its
    # own column (§6.2). One that is not a boundary is one outside the range — and then the
    # number does not matter, because no coordinate can be on the other side of it. A
    # number inside the axis that is not a boundary is the certificate reading the cell as
    # something the file does not say (§15.99).
    lo, hi = axis_span(axis)
    v = num(value) if value is not None else None
    if v is None or lo is None or hi is None or (lo <= v <= hi):
        raise Bad(f"{where}: the file writes `{text}`, and the certificate reads it as a "
                  f"number the axis has no boundary for")
    return 1


STATED = []


def check(cert):
    """Re-check one rule's certificate. Returns (lines, ok)."""
    out = [f"{cert['rule']} ({cert['alias']} v{cert['version']}, sha256:{cert['source_sha256'][:12]}) "
           f"— certificate by rulec {cert['rulec']}"]
    ok = True
    global GROUPS
    GROUPS = cert.get("groups", {})
    ranges = {k: (v[0], v[1]) for k, v in cert.get("ranges", {}).items()}
    try:
        n, unread = check_types(cert)
        rest = f", {unread} rest on a leaf this program cannot type" if unread else ""
        if unread:
            STATED.append(f"{unread} values whose units rest on a leaf this program cannot type")
        out.append(f"  units: {n} values keep the type the rule declares{rest}")
    except Bad as e:
        out.append(f"  FAILED units: {e}")
        ok = False
    try:
        checked, none_stated = check_values(cert)
        rest = f", {none_stated} with no interval to claim" if none_stated else ""
        out.append(f"  int64: {checked} values fit{rest}")
    except Bad as e:
        out.append(f"  FAILED int64: {e}")
        ok = False
    # The reach of a derived value is the interval its own expression is forced into — the
    # cover's "out of reach" leaves are held to that, recomputed here.
    reach = {}
    for v in cert.get("values", []):
        got = interval(v["expr"], {**ranges, **{k: (str(a), str(b)) for k, (a, b) in reach.items()}})
        if got is not None:
            reach[v["name"]] = got
    if not cert["tables"]:
        out.append("  no table states a certificate")
    for t in cert["tables"]:
        t["_reach_of"] = reach
        t["_tables"] = cert["tables"]
        t["_ranges"] = cert.get("ranges", {})
        t["_sieve"] = True
        t["_cert"] = cert
        for what, n in (("pairs the axes do not part", len(t.get("undecided", []))),
                        ("rows an `apply` brought in and this rule leaves unused", len(t.get("unused", []))),
                        ("rows the sieve rules out", len(t.get("unreachable", [])))):
            if n:
                STATED.append(f"{t['table']}: {n} {what}")
        try:
            out.append("  " + check_table(t))
        except Bad as e:
            out.append(f"  FAILED {e}")
            ok = False
    return out, ok


def main(argv):
    rule = None
    if "--rule" in argv:
        i = argv.index("--rule")
        if i + 1 >= len(argv):
            print("--rule wants a file", file=sys.stderr)
            return 2
        rule = argv[i + 1]
        argv = argv[:i] + argv[i + 2:]
    texts = []
    if argv:
        for p in argv:
            with open(p) as fh:
                texts.append((p, fh.read()))
    else:
        texts.append(("<stdin>", sys.stdin.read()))
    worst = 0
    for where, text in texts:
        for line in (l for l in text.splitlines() if l.strip()):
            try:
                cert = json.loads(line)
                lines, ok = check(cert)
            except (ValueError, KeyError, TypeError) as e:
                print(f"{where}: not a certificate this program can read: {e}", file=sys.stderr)
                worst = max(worst, 2)
                continue
            if rule is not None:
                with open(rule, "rb") as fh:
                    digest = hashlib.sha256(fh.read()).hexdigest()
                if digest == cert["source_sha256"]:
                    lines.append(f"  the digest is {rule}'s")
                    try:
                        read, loose, apart = check_cells(cert, rule)
                        rest = f", {loose} literals not pinned to a boundary" if loose else ""
                        rest += f", {apart} rows written in an applied rule" if apart else ""
                        if loose:
                            STATED.append(f"{loose} literals written in a unit this program cannot read")
                        if apart:
                            STATED.append(f"{apart} rows written in an applied rule, and read back in its own certificate")
                        lines.append(f"  the file says the same: {read} cells read back from it{rest}")
                    except Bad as e:
                        lines.append(f"  FAILED {e}")
                        ok = False
                else:
                    lines.append(f"  FAILED the certificate is about another text than {rule}")
                    ok = False
            if rule is None:
                STATED.append("the certificate is held to no text: pass `--rule <file.rule>`")
            if ok:
                lines.append("  every claim this program states was proved"
                             if not STATED else
                             "  stated rather than proved: " + "; ".join(STATED))
            print("\n".join(lines))
            STATED.clear()
            if not ok:
                worst = max(worst, 1)
    return worst


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
