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
  * **max and min** (§15.219): the range stated for what a `max` or a `min` answers holds
    its column's range and its `empty` value, since it answers one of those.
  * **fold answers** (§15.219): the range stated for a fold's answer that the items below
    read holds every value the fold answers with, worked out here from their expressions.
  * **open values** (§15.227): the range an open value declares holds what its terms come to
    together — added, multiplied, the largest or the smallest — each term's interval worked out
    here as any other value's is.
  * **chances** (§15.224–§15.226, §15.243): every claim of a `chances` table is counted again.
    Box by box: the coordinates of each random axis tile what its input is drawn from, a box
    holds the product of its lengths, what the rows answer in it is worked out here from the
    tables, a class's boxes add up to its points and hits, and an axis that is not random covers
    what its column takes, cut on a step that divides the one its values sit on. Point by point:
    every draw of every combination the claim's cells let in is run here through the tables and
    the values the rule computes. By weight: the walk of the pick is run here at every draw, over the
    sequence the certificate carries. The hits over the points have to pass the claim.

What it checks, per table:

  * every row's **box** is the one its own cells describe, recomputed here from the parsed
    cells and the axes — less the coordinates of a column decided above at a value no row of
    the deciding table writes, each of them a `never` fact of the table's, earned back from
    those rows (rulec's §15.253): a row that writes another word, or one that passes on a column
    of its own table (`passes`) whose box there takes no such value (§15.261). A box wider or
    narrower than that is refused. Every word a row is said to write is a value of its column and
    no name of the rule, which a cell reads as the name (§15.261).
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
    `constraint` that cannot hold there, a derived value whose coordinate lies outside
    what the derive can produce, or multipliers that add the box's ends, the derives and
    the constraints up to a contradiction (§15.141). All of those are re-checked here.
  * every numeric axis **tiles**, and on the step its column's values really take
    (§15.190): the coordinates run from one end of the range to the other, each touching the
    next or one step past it, and that step has to divide the one worked out here from the
    values' expressions and the `scales` the certificate states — a column of tenths of a
    pound cut every whole pound leaves the values between two coordinates to no row. Two
    intervals side by side, or one point written twice, are refused (§15.243): the first leaves
    the value they meet at to no row, the second puts it on two coordinates.

What it checks, per contract (§15.142): the rule's door — each numeric input's declared
range at the scale the value travels at, each `constraint` between two of them, each enum
input's values — is built again here from the rule, and every case of the contract's
condition has to keep each thing the door asks: by multipliers that add the case and the
thing's negation up to a contradiction, by strings or truth values the case cannot all
have, or by an enum input's strings all being the enum's. The condition itself, and how it
opens into cases, is the certificate's reading of the contract.

Given `--rule <file.rule>` it also checks that the certificate is **about that text**: the
digest matches, and every cell of every row is read back out of the file at the byte span
the certificate names and held beside the parsed form it states. Each contract, named
relative to the rule, has to have the digest the certificate gives for it, and so does every
contribution merged into the rule (`contributions`, §15.243), whose rows are read back out of
its own file. A contribution's certificate is the certificate of the rule it is merged into,
so `--rule` names that rule. The cells of every claim of `chances` and of every `sequence` a
pick by weight walks are read back too. A literal is
turned into a number by looking it up among the axis's own boundary coordinates, so no unit
table is needed here; a literal that is not one of them is counted and reported rather than
passed.

A leaf that rests on the tables above is re-checked too, and only with what the certificate
carries: the columns each table decides, the value each of its rows writes, and the boxes
those rows take are all here, so the span a value puts an input into is recomputed rather
than believed. Such a leaf has to come with its reason or it is refused.

What it does **not** check: the pairs the axes do not part, and a claim of `chances` the
check left undecided within its budget (W129), which are listed rather than proved.

Exit code 0 when every table holds, 1 when one does not, 2 on a certificate it cannot
read. No dependencies; Python 3.9 or later.
"""

import hashlib
import itertools
import json
import math
import os
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


def side(x):
    """One side of a `constraint`: the names it adds up. A side of one name is written as the
    name, and a side that adds several as the list of them (rulec's §15.216)."""
    return list(x) if isinstance(x, list) else [x]


def side_text(x):
    return " + ".join(side(x))


def guarantees(cert):
    """The `a <= b` pairs the `constraint` lines promise, closed under chains: two lines
    saying a running total never passes the next, and that one never passes the whole,
    together say the first never passes the whole. rulec reads them that way, so a
    re-checker that did not would refuse an honest certificate. A line with a sum on a side
    says nothing of one name against another, and is left out."""
    le = set()
    for k in cert.get("constraints", []) or []:
        if isinstance(k.get("left"), list) or isinstance(k.get("right"), list):
            continue
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
    if "inside" in e:
        # What a row writes when it names one of its table's own columns (rulec's §15.213): the
        # name's interval cut to what the row's own cell on that column lets through, worked out
        # here from the cell (`row_cell_bounds`), never from the certificate's word for it.
        got = interval(e["inside"], ranges, le)
        cut = e.get("cell")
        if got is None or cut is None:
            return got
        lo = got[0] if cut[0] is None else max(got[0], cut[0])
        hi = got[1] if cut[1] is None else min(got[1], cut[1])
        return (lo, hi) if lo <= hi else got
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
            # Both ends on one side of zero: an interval written upside down has no quotient.
            if e["op"] == "/" and not (bl > 0 and bh > 0 or bl < 0 and bh < 0):
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
        # A remainder by a positive whole constant (rulec's §15.214): of the whole part, inside one
        # period the whole part's interval moved down by the multiples below it, otherwise the
        # whole of 0 to the constant less one.
        if e["call"] == "mod" and len(args) == 2:
            k = interval(args[1], ranges, le)
            if k is None or k[0] != k[1] or k[0].denominator != 1 or k[0] <= 0:
                return None
            n = k[0].numerator
            a, b = math.floor(inner[0]), math.floor(inner[1])
            if a // n == b // n:
                base = (a // n) * n
                return (Fraction(a - base), Fraction(b - base))
            return (Fraction(0), Fraction(n - 1))
        # The whole square root of the whole part (rulec's §15.215): it rises with its argument,
        # so its ends are the roots of the argument's; below zero there is none.
        if e["call"] == "isqrt" and len(args) == 1:
            if inner[0] < 0:
                return None
            return (Fraction(math.isqrt(math.floor(inner[0]))), Fraction(math.isqrt(math.floor(inner[1]))))
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


def truth_of(e, ranges, le=frozenset()):
    """The truth value a comparison always takes over the intervals of its two sides, or None
    when it can take either, or a side has no interval (§15.196). What `rulec` reads a boolean
    `define` of one comparison with: `made <= 2027-03-31` with `made` declared up to the end of
    2026 is always true."""
    if not isinstance(e, dict) or e.get("op") not in ("<=", "<", ">=", ">"):
        return None

    def side(x):
        # A date literal stands for its day number, on the scale `ranges` gives a date input.
        if isinstance(x, dict) and x.get("type") == "date" and "lit" in x and x.get("value") is not None:
            v = num(x["value"])
            return (v, v)
        return interval(x, ranges, le)

    a, b = side(e["l"]), side(e["r"])
    if a is None or b is None:
        return None
    (al, ah), (bl, bh) = a, b
    op = e["op"]
    if op == "<=":
        return True if ah <= bl else (False if bh < al else None)
    if op == "<":
        return True if ah < bl else (False if bh <= al else None)
    if op == ">=":
        return True if bh <= al else (False if ah < bl else None)
    return True if bh < al else (False if ah <= bl else None)


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
        # A remainder and a whole square root are of a number, and are one (rulec's §15.214,
        # §15.215).
        if e["call"] in ("mod", "isqrt"):
            if args[0] != "number":
                raise Bad(f"{e['call']}(`{args[0]}`): it takes a number")
            return "number"
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


def numeric_outputs(cert):
    """For every column a table writes numbers into, what its rows write there, as expressions
    (§15.196): `None` where a row writes something this program cannot read. And the columns
    whose rows state nothing they write — a certificate from before they did. An optional column
    holds a number where it has a value (rulec's §15.249): it is read as the type it wraps, and a
    row whose answer cell is `none` — the word read back out of the file — writes no number."""
    types = cert.get("types", {})
    outs, unstated = {}, []
    for t in cert.get("tables", []):
        rows = t.get("rows", [])
        for oi, col in enumerate(t.get("decides", [])):
            ty = types.get(col) or ""
            optional = ty.endswith("?")
            if not numeric(ty[:-1] if optional else ty):
                continue
            if not any("writes" in r for r in rows):
                if col not in unstated:
                    unstated.append(col)
                continue
            if col in outs and outs[col] is None:
                continue
            es = outs.setdefault(col, [])
            for r in rows:
                ws = r.get("writes") or []
                w = ws[oi] if oi < len(ws) else None
                ps = r.get("produces") or []
                if optional and w is None and oi < len(ps) and ps[oi] == "none":
                    continue
                if not isinstance(w, dict) or not isinstance(w.get("expr"), dict):
                    outs[col] = None
                    break
                e = w["expr"]
                if "inside" in w:
                    # The row says its own cell narrows what it writes (rulec's §15.213). What the
                    # cell lets through is worked out here from the cell; what the row says is
                    # held to it in `check_outputs`.
                    name = e.get("name")
                    e = {"inside": e, "said": w["inside"], "row": r.get("row"), "table": t.get("table"),
                         "cell": None if name is None else row_cell_bounds(cert, t, r, name)}
                es.append(e)
    return outs, unstated


def row_cell_bounds(cert, t, row, name):
    """The values row `row` of table `t` lets its column `name` take, read from its own cell on
    that column (rulec's §15.213): the ends of a comparison — a strict one moved in to the next
    point of the grid the column's values sit on (`column_grid`) — the value of an `=`, the hull
    of a set of values. `(lo, hi)`, an end None where the cell leaves it open; None when `name`
    is not a column of the table or the cell says nothing an interval can."""
    axes = t.get("axes", [])
    ai = next((i for i, a in enumerate(axes) if a.get("column") == name), None)
    tests = row.get("tests") or []
    if ai is None or ai >= len(tests):
        return None
    cell = tests[ai]
    if cell.get("cell") == "in":
        vals = [num(v) for v in cell.get("values", [])]
        if not vals or any(v is None for v in vals):
            return None
        return (min(vals), max(vals))
    if cell.get("cell") != "cmp":
        return None
    g = column_grid(cert, name)
    lo = hi = None
    for x in cell.get("tests", []):
        v, op = num(x.get("value")), x.get("op")
        if v is None:
            return None
        # Without a grid a strict end stays where it is: the interval is wider, never wrong.
        up = v if g is None or g <= 0 else (v // g) * g + g
        down = v if g is None or g <= 0 else -((-v) // g) * g - g
        ends = {"=": (v, v), ">=": (v, None), ">": (up, None), "<=": (None, v), "<": (None, down)}.get(op)
        if ends is None:
            return None
        if ends[0] is not None:
            lo = ends[0] if lo is None else max(lo, ends[0])
        if ends[1] is not None:
            hi = ends[1] if hi is None else min(hi, ends[1])
    return (lo, hi)


def worked_ranges(cert):
    """The interval every value the rule computes and every column a table writes numbers into
    is forced into, worked out here: a value from its expression, a table's column from what
    each of its rows writes (§15.196). A name the rule declares is bounded by its declared range;
    a column a table writes numbers into, never by the certificate's word for it. Returns the
    intervals, the columns' expressions, and the columns whose rows state nothing."""
    le = guarantees(cert)
    stated = {k: (num(v[0]), num(v[1])) for k, v in cert.get("ranges", {}).items()}
    values = cert.get("values", [])
    outs, unstated = numeric_outputs(cert)
    known = {}

    def env():
        r = {k: (None if a is None else str(a), None if b is None else str(b))
             for k, (a, b) in stated.items() if k not in outs}
        r.update({k: (str(a), str(b)) for k, (a, b) in known.items()})
        return r

    # What depends on what runs both ways — a `define` over a table's column, a table writing a
    # `define` — so the two are worked out together until nothing more can be.
    for _ in range(len(values) + len(outs) + 1):
        moved = False
        for v in values:
            if v["name"] in known:
                continue
            got = interval(v["expr"], env(), le)
            if got is not None:
                known[v["name"]] = got
                moved = True
        for col, es in outs.items():
            if col in known or not es:
                continue
            e = env()
            ivs = [interval(x, e, le) for x in es]
            if all(i is not None for i in ivs):
                known[col] = (min(i[0] for i in ivs), max(i[1] for i in ivs))
                moved = True
        if not moved:
            break
    return known, outs, unstated


def check_outputs(cert, known, outs):
    """The range the certificate states for a column a table writes numbers into holds every
    value its rows write (§15.196). Returns how many were held to that."""
    stated = cert.get("ranges", {})
    n = 0
    env = {k: (None if a is None else str(a), None if b is None else str(b))
           for k, (a, b) in ((k, (num(v[0]), num(v[1]))) for k, v in stated.items()) if k not in outs}
    env.update({k: (str(a), str(b)) for k, (a, b) in known.items()})
    for col, es in outs.items():
        for e in es or []:
            if "inside" not in e:
                continue
            if "name" not in e["inside"]:
                raise Bad(f"{e['table']}: row {e['row']} says it writes {col} only inside a range, and what it writes is no name")
            if e["cell"] is None:
                raise Bad(f"{e['table']}: row {e['row']} says its cell on {e['inside']['name']} narrows what it writes, "
                          f"and {e['inside']['name']} is no column of the table it can read a range from")
            got = interval(e, env, guarantees(cert))
            said = e.get("said")
            a, b = (num(said[0]), num(said[1])) if isinstance(said, list) and len(said) == 2 else (None, None)
            if got is None or a is None or b is None or a > got[0] or b < got[1]:
                shown = "no interval" if got is None else f"[{got[0]}, {got[1]}]"
                raise Bad(f"{e['table']}: row {e['row']}'s cell lets {e['inside']['name']} take {shown}, "
                          f"and the certificate says the row writes it inside [{a}, {b}]")
    for col, es in outs.items():
        s = stated.get(col)
        if col not in known:
            if s is not None:
                raise Bad(f"{col}: the certificate states a range for it, and this program cannot work one out from what its rows write")
            continue
        if s is None:
            continue
        lo, hi = known[col]
        a, b = num(s[0]), num(s[1])
        if a is None or b is None or a > lo or b < hi:
            raise Bad(f"{col}: its rows write values from {lo} to {hi}, and the certificate states the range [{s[0]}, {s[1]}]")
        n += 1
    return n


def check_values(cert, known):
    """int64: hold each value's interval, worked out here, to the one stated and to i64."""
    declared = {k: (num(v[0]), num(v[1])) for k, v in cert.get("ranges", {}).items()}
    checked = none_stated = 0
    for v in cert.get("values", []):
        got = known.get(v["name"])
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


def check_extremes(cert, known):
    """What a `max` or a `min` answers is a value of its column, or its `empty` value when it
    looks at no element (§15.219). The range `ranges` states for it is worked out from those two
    rather than declared, and a table's axis on it is cut over that range: a range that left out
    a value it can answer would leave that value covered by nothing. So the stated range has to
    hold the column's — the declared one, or the interval worked out here for a value of each
    element — and the `empty` value. An enum's answer is one of its values, `empty` included,
    and it states no range. Returns how many were held."""
    types = cert.get("types", {})
    stated = cert.get("ranges", {})
    enums = cert.get("enums", {})
    walks = cert.get("walks", [])
    n = 0
    for e in cert.get("extremes", []):
        name, col, kind = e.get("name"), e.get("of"), e.get("kind")
        if kind not in ("max", "min"):
            raise Bad(f"{name}: the certificate calls it a `{kind}`, which is neither `max` nor `min`")
        if name not in walks:
            raise Bad(f"{name}: a `{kind}` is a value a walk leaves behind, and `walks` does not name it")
        ty = types.get(col)
        if ty is None or types.get(name) != ty:
            raise Bad(f"{name}: a `{kind}` of {col} answers with {col}'s type {ty}, and the certificate says {types.get(name)}")
        base = ty.rstrip("?")
        if base in enums:
            if e.get("empty") not in enums[base]:
                raise Bad(f"{name}: its `empty` value {e.get('empty')} is not a value of {base}")
            if name in stated:
                raise Bad(f"{name}: a `{kind}` of an enum answers one of its values, and the certificate states a range for it")
            n += 1
            continue
        s = stated.get(name)
        if s is None:
            # No range is claimed for the answer, so no axis is cut over one.
            n += 1
            continue
        if col in known:
            lo, hi = known[col]
        elif col in stated:
            lo, hi = num(stated[col][0]), num(stated[col][1])
        else:
            raise Bad(f"{name}: the certificate states a range for it, and this program cannot work out the range of {col}")
        if e.get("empty") is None:
            raise Bad(f"{name}: a `{kind}` of numbers states no `empty` value")
        z = num(e["empty"])
        want_lo = None if lo is None else min(lo, z)
        want_hi = None if hi is None else max(hi, z)
        a, b = num(s[0]), num(s[1])
        if (want_lo is None and a is not None) or (want_hi is None and b is not None) \
                or (a is not None and a > want_lo) or (b is not None and b < want_hi):
            shown = lambda v: "open" if v is None else str(v)
            raise Bad(f"{name}: a `{kind}` of {col} answers from {shown(want_lo)} to {shown(want_hi)} with its `empty` value, "
                      f"and the certificate states the range [{s[0]}, {s[1]}]")
        n += 1
    return n


def check_opens(cert, known):
    """An open value (rulec's §15.227) is its terms put together — added, multiplied, the largest
    or the smallest, starting from its `empty` value — and the range `ranges` states for it is
    declared: the tables below are checked over the whole of it, and the claim is that the terms,
    the rule's and every contribution's, together stay inside it (E131). Each term's interval is
    worked out here, or declared for an input, and the combination is held to the stated range.
    Returns (open values held, terms, terms from contributions)."""
    stated = cert.get("ranges", {})
    held = terms = by = 0
    for o in cert.get("opens", []):
        name, how = o.get("name"), o.get("combine")
        if how not in ("sum", "product", "max", "min"):
            raise Bad(f"{name}: `combine {how}` is not one of sum, product, max and min")
        s = stated.get(name)
        if s is None or num(s[0]) is None or num(s[1]) is None:
            raise Bad(f"{name}: an open value declares its range, and the certificate states none")
        ivs = []
        if o.get("empty") is not None:
            z = num(o["empty"])
            ivs.append((z, z))
        elif how == "sum":
            ivs.append((Fraction(0), Fraction(0)))
        elif how == "product":
            ivs.append((Fraction(1), Fraction(1)))
        else:
            raise Bad(f"{name}: a `combine {how}` states no `empty` value")
        for t in o.get("terms", []):
            n = t.get("name")
            terms += 1
            if t.get("from"):
                by += 1
            if n in known:
                ivs.append(known[n])
            elif n in stated and num(stated[n][0]) is not None and num(stated[n][1]) is not None:
                ivs.append((num(stated[n][0]), num(stated[n][1])))
            else:
                raise Bad(f"{name}: this program cannot work out the range of its term {n}")
        lo, hi = ivs[0]
        for a, b in ivs[1:]:
            if how == "sum":
                lo, hi = lo + a, hi + b
            elif how == "product":
                cs = [lo * a, lo * b, hi * a, hi * b]
                lo, hi = min(cs), max(cs)
            elif how == "max":
                lo, hi = max(lo, a), max(hi, b)
            else:
                lo, hi = min(lo, a), min(hi, b)
        a, b = num(s[0]), num(s[1])
        if lo < a or hi > b:
            raise Bad(f"{name}: its terms together come to [{lo}, {hi}], outside the range [{s[0]}, {s[1]}] it declares")
        held += 1
    for t in cert.get("tables", []):
        for ax in t.get("axes", []):
            if ax.get("kind") == "open" and not any(o.get("name") == ax.get("column") for o in cert.get("opens", [])):
                raise Bad(f"{t['table']}: {ax.get('column')} is called an open value, and `opens` does not name it")
    return held, terms, by


def check_fold_answers(cert, known):
    """The answer of a fold the items below read is one of the values the fold answers with
    (§15.219): `empty`, `exhausted` unless it is `held`, a `stop with`, what an arm takes or
    keeps — `held` is one of those. The range `ranges` states for it is worked out from them,
    and what reads the answer is bounded by it, so it has to hold the interval of every one,
    worked out here from its expression. Returns how many were held."""
    stated = cert.get("ranges", {})
    env = {k: (v[0], v[1]) for k, v in stated.items()}
    env.update({k: (str(a), str(b)) for k, (a, b) in known.items()})
    n = 0
    for fa in cert.get("fold_answers", []):
        name = fa.get("name")
        s = stated.get(name)
        if s is None:
            raise Bad(f"{name}: the certificate lists the values the fold answers it with, and states no range for it")
        lo = hi = None
        for e in fa.get("answers", []):
            if e.get("type") == "date" and "lit" in e:
                v = num(e.get("value"))
                iv = None if v is None else (v, v)
            else:
                iv = interval(e, env, guarantees(cert))
            if iv is None:
                raise Bad(f"{name}: one of the values the fold answers with has no interval this program can work out")
            lo = iv[0] if lo is None else min(lo, iv[0])
            hi = iv[1] if hi is None else max(hi, iv[1])
        if lo is None:
            raise Bad(f"{name}: the fold answers with no value at all")
        a, b = num(s[0]), num(s[1])
        if a is None or b is None or a > lo or b < hi:
            raise Bad(f"{name}: the fold answers with values from {lo} to {hi}, and the certificate states the range [{s[0]}, {s[1]}]")
        n += 1
    return n


def column_kinds(cert):
    """What kind of column each name makes (§15.196): a `derive` is `derived`, a `define` of any
    type `define`, an input `input`, what a walk leaves behind (`count`, `sum`) `walk`; anything
    else — a column a table above decides, a field of the elements a fold walks — `upstream`."""
    out = {}
    for v in cert.get("values", []):
        if v.get("of") == "derive":
            out[v["name"]] = "derived"
        elif v.get("of") == "define":
            out[v["name"]] = "define"
    for n in cert.get("inputs", []):
        out.setdefault(n, "input")
    for n in cert.get("walks", []):
        out.setdefault(n, "walk")
    # An open value's range is declared, as an input's is (rulec's §15.227).
    for o in cert.get("opens", []):
        out.setdefault(o.get("name"), "open")
    return out


def check_kinds(t, kinds):
    """Each axis says the kind of column it is, and it is the kind worked out here: the reach of a
    point and the tiling of an axis are read by kind, so a column called by the wrong one would be
    read the wrong way."""
    for a in t["axes"]:
        want = kinds.get(a["column"], "upstream")
        if a.get("kind") != want:
            raise Bad(f"{t['table']}: {a['column']} is a column of kind `{want}`, and the certificate calls it `{a.get('kind')}`")


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
    (§15.140). A side that adds names up reaches from the sum of their low ends to the sum of
    their high ends (rulec's §15.216)."""
    def ends(names):
        lo, hi = Fraction(0), Fraction(0)
        for n in names:
            a, b = span_of_name(t, n, path)
            lo = None if lo is None or a is None else lo + a
            hi = None if hi is None or b is None else hi + b
        return lo, hi
    (ll, lh) = ends(side(k["left"]))
    (rl, rh) = ends(side(k["right"]))
    shut = any(left_out(t, n, path) for n in side(k["left"]) + side(k["right"]))
    return {
        "<=": ll is not None and rh is not None and (ll > rh or (ll == rh and shut)),
        "<": ll is not None and rh is not None and ll >= rh,
        ">=": lh is not None and rl is not None and (lh < rl or (lh == rl and shut)),
        ">": lh is not None and rl is not None and lh <= rl,
    }[k["op"]]


def derived_rules_out(t, ai, path):
    """Whether the coordinate of a column the rule computes — a derive, or a `define` of a
    number (§15.195) — lies outside what its own expression can produce, the interval this
    program works out for it. An interval coordinate leaves its ends out, so one that starts
    where the reach ends lies outside it. `None` when this program has no expression for it."""
    if ai >= len(path) or ai >= len(t["axes"]):
        return None
    if t["axes"][ai].get("kind") == "input":
        return None
    reach = t.get("_reach_of", {}).get(t["axes"][ai]["column"])
    if reach is None:
        return None
    lo, hi = bound_at(t, ai, path[ai])
    point = lo is not None and hi is not None and lo == hi
    if point:
        return hi < reach[0] or lo > reach[1]
    return (hi is not None and hi <= reach[0]) or (lo is not None and lo >= reach[1])


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


def passed_axis(u, r, oi):
    """The axis of the column of its own table a row passes on into its output `oi` (`passes`,
    rulec's §15.261): one of the table's axes whose coordinates are words. None where it names
    none, or names what this program cannot read."""
    ps = r.get("passes")
    a = ps[oi] if isinstance(ps, list) and oi < len(ps) else None
    if not isinstance(a, int) or isinstance(a, bool) or not 0 <= a < len(u["axes"]):
        return None
    axis = u["axes"][a]
    if numeric_axis(axis) or axis.get("prefixes") is not None or not all(isinstance(x, str) for x in axis["coords"]):
        return None
    return a


def never_written(t, u, oi, col, val):
    """No row of the table that decides a column writes a value at all: each writes another
    word, or passes on a column of its own table whose box there takes no such value — the row
    writes what that column holds where it fires, and it fires only inside its box (rulec's
    §15.261)."""
    for r in u["rows"]:
        p = r.get("produces")
        if not isinstance(p, list) or oi >= len(p):
            raise Bad(f"{t['table']}: row {r['row']} of {u['table']} writes something this "
                      f"program cannot read, so «{col} is never {val}» cannot be settled")
        if p[oi] is None:
            a = passed_axis(u, r, oi)
            if a is None:
                raise Bad(f"{t['table']}: row {r['row']} of {u['table']} writes something this "
                          f"program cannot read, so «{col} is never {val}» cannot be settled")
            coords = u["axes"][a]["coords"]
            if val in coords and coords.index(val) in r["accepts"][a]:
                raise Bad(f"{t['table']}: row {r['row']} of {u['table']} passes on {u['axes'][a]['column']}, "
                          f"which can be {val} there, so it is not a value the tables above never reach")
            continue
        if p[oi] == val:
            raise Bad(f"{t['table']}: row {r['row']} of {u['table']} does write {col} = {val}, "
                      f"so it is not a value the tables above never reach")
    return True


def check_words(t):
    """Every word a row is said to write is a value of the column it is written into, and no name of
    the rule. rulec reads a word in an answer cell as a name wherever the rule declares one spelled
    so, and the row then writes whatever the name holds (rulec's §3.2). Without this, a row that
    passes a name on could be said to write the name as a word — the file does write that word
    there — and a value it does write could then be called one no row writes (rulec's §15.261)."""
    cert = t.get("_cert") or {}
    types, enums = cert.get("types", {}), cert.get("enums", {})
    decides = t.get("decides") or []
    for r in t["rows"]:
        for j, w in enumerate(r.get("produces") or []):
            if w is None:
                continue
            col = decides[j] if j < len(decides) else None
            ty = types.get(col) if isinstance(col, str) else None
            if not isinstance(ty, str):
                raise Bad(f"{t['table']}: row {r['row']} is said to write `{w}` into a column the certificate gives no type for")
            base = ty[:-1] if ty.endswith("?") else ty
            values = list(enums.get(base) or (["true", "false"] if base == "bool" else []))
            if ty.endswith("?"):
                values.append("none")
            if w not in values:
                raise Bad(f"{t['table']}: row {r['row']} is said to write `{w}` into {col}, which is no value of {ty}")
            if w in types:
                raise Bad(f"{t['table']}: row {r['row']} is said to write the word `{w}` into {col}, and `{w}` is a name "
                          f"of the rule, which a cell reads as what the name holds")


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


def days_of_axis(t, ai):
    """The days an axis of inputs takes, when the input's range is a date of a koyomi file
    (`range from koyomi`, §15.174): the certificate's `days` for its column. None for any
    other axis."""
    axis = t["axes"][ai]
    if axis.get("kind") != "input":
        return None
    d = t.get("_days", {}).get(axis["column"])
    return None if d is None else sorted(num(x) for x in d["days"])


def day_rules_out(t, ai, path):
    """Whether the coordinate the path takes on an axis of koyomi's days holds none of them
    (§15.174). `None` when the axis has no days."""
    if ai >= len(path) or ai >= len(t["axes"]):
        return None
    days = days_of_axis(t, ai)
    if days is None:
        return None
    lo, hi = closed_at(t, ai, path[ai])
    return not any((lo is None or lo <= d) and (hi is None or d <= hi) for d in days)


def truth_rules_out(t, ai, path):
    """Whether the path takes, on the axis of a boolean `define`, the truth value the define
    never takes over the ranges it is read with — worked out here from its comparison
    (§15.196)."""
    if ai >= len(path) or ai >= len(t["axes"]):
        return False
    tv = t.get("_truth", {}).get(t["axes"][ai]["column"])
    if tv is None:
        return False
    label = t["axes"][ai]["coords"][path[ai]]
    return label in ("true", "false") and (label == "true") != tv


def point_ruled_out(t, path):
    """Whether the sieve rules this point out: some constraint cannot hold at it, some
    derived column cannot reach it, a boolean `define` stands at the truth value it never
    takes, it takes no day of koyomi's on an axis of them, or a fact of this table's about the
    tables above rules it out — a value they never write, or two they never hold together, each
    earned back by `check_above` (rulec's §15.253; the Lean program's `pointRuledOut` reads the
    same)."""
    for k in t.get("constraints", []):
        if constraint_rules_out(t, k, path):
            return True
    if any(day_rules_out(t, ai, path) for ai in range(len(path))):
        return True
    if any(truth_rules_out(t, ai, path) for ai in range(len(path))):
        return True
    if above_rules_out(t, path):
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
# `derive`'s equation, a `define`'s whose expression is linear, a declared range, a `constraint`
# — and this program builds each one again from the rule's own values, ranges and constraints. A refutation is then a list of
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
        # A `derive`'s equation, or a `define`'s whose expression is linear (rulec's §15.209): the
        # same equation, built from the value's own expression.
        eq = f.get("derive", f.get("define"))
        if eq is not None:
            of = "derive" if "derive" in f else "define"
            v = values.get(eq)
            if v is None:
                raise Bad(f"{t['table']}: the model names the equation of {eq}, which states no expression")
            if v.get("of") not in (None, of):
                raise Bad(f"{t['table']}: the model names the equation of {eq} as a {of}, and it is a {v.get('of')}")
            got = linearize(v["expr"], v.get("type"), types)
            if got is None:
                raise Bad(f"{t['table']}: the model takes {eq} as linear, and its expression is not")
            terms = {n: -c for n, c in got[0].items()}
            terms[eq] = terms.get(eq, Fraction(0)) + 1
            k = -got[1]
            if not f["le"]:
                terms, k = {n: -c for n, c in terms.items()}, -k
            out.append((terms, k, False))
        elif "range" in f:
            # A value the rule computes is bounded by the interval worked out here from its
            # expression, not by the document's word for it (§15.195); an input by its range.
            got = t.get("_reach_of", {}).get(f["range"])
            if got is None:
                # A column a table writes numbers into is bounded by what its rows write, worked
                # out here, not by the document's word for it (§15.196).
                got = t.get("_written", {}).get(f["range"])
            r = cert.get("ranges", {}).get(f["range"]) if got is None else (str(got[0]), str(got[1]))
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
            d = {}
            for n in side(k["left"]):
                d[n] = d.get(n, Fraction(0)) + 1
            for n in side(k["right"]):
                d[n] = d.get(n, Fraction(0)) - 1
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


def farkas_holds(t, cert, refs, box, above=None):
    """Whether the multipliers refute the model inside the box. Returns None when they do,
    or what is wrong.

    `above` is a row of a table above, `(that table, the row)`, whose box lends its ends: an
    end on a numeric column it cuts (`cond`) holds of every coordinate the row takes there,
    and the column has to be one this table numbers a value for (§15.195)."""
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
        elif "cond" in ref:
            if above is None:
                return "it names the end of a row above, and no row above is in question"
            u, row = above
            cr = ref["cond"]
            name, hi, at, left_out = cr.get("name"), cr.get("hi"), num(cr.get("at")), bool(cr.get("open"))
            ua = axis_of(u, name) if isinstance(name, str) else None
            if ua is None or at is None or not numeric_axis(u["axes"][ua]):
                return f"it names an end of {name}, which {u['table']} does not cut as a number"
            if not numbered(t, name):
                return f"it names {name}, which this table numbers no value for"
            if not beyond(u, ua, row["accepts"][ua], hi, at, left_out):
                return f"a coordinate row {row['row']} of {u['table']} takes on {name} lies beyond the end it names"
            terms, fk, fs = ({name: Fraction(1)}, -at, left_out) if hi else ({name: Fraction(-1)}, at, left_out)
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


# --- A contract, held to the rule's door (§15.142) -----------------------------------------
#
# What the certificate says about a contract is its condition on the values the rule reads,
# opened into cases, and for each thing the door asks of those values — an end of an input's
# range, a `constraint`, an enum's values — why every case keeps it. The door is built again
# here from the rule's ranges, constraints and enums; what is taken from the certificate is
# the reading of the contract and the proofs.

def contract_doors(cert, k):
    """The door, built from the rule: `{key: (terms, k, strict)}` for a numeric condition that
    has to hold (`sum + k <= 0`), `{key: values}` for an enum input's values."""
    scales = {i["input"]: Fraction(str(i["scale"])) for i in k["inputs"]}
    out = {}
    for inp, sc in scales.items():
        if inp not in k["vars"]["num"]:
            raise Bad(f"contract {k['shape']}: {inp} is fed by the contract and missing from its values")
        r = cert.get("ranges", {}).get(inp)
        if r is None:
            continue
        lo, hi = num(r[0]), num(r[1])
        if lo is not None:
            out[("range", inp, False)] = ({inp: Fraction(-1)}, lo * sc, False)
        if hi is not None:
            out[("range", inp, True)] = ({inp: Fraction(1)}, -hi * sc, False)
    for i, c in enumerate(cert.get("constraints", [])):
        if any(n not in scales for n in side(c["left"]) + side(c["right"])):
            continue
        d = {}
        for n in side(c["left"]):
            d[n] = d.get(n, Fraction(0)) + 1 / scales[n]
        for n in side(c["right"]):
            d[n] = d.get(n, Fraction(0)) - 1 / scales[n]
        if c["op"] in (">=", ">"):
            d = {n: -v for n, v in d.items()}
        out[("constraint", i)] = (d, Fraction(0), c["op"] in ("<", ">"))
    enums = cert.get("enums", {})
    types = cert.get("types", {})
    for inp in k["vars"]["str"]:
        if types.get(inp) in enums:
            out[("member", inp)] = enums[types[inp]]
    return out


def door_text(cert, key):
    """One thing the door asks, in words."""
    if key[0] == "range":
        return f"the {'high' if key[2] else 'low'} end of {key[1]}'s range"
    if key[0] == "constraint":
        c = cert["constraints"][key[1]]
        return f"constraint {side_text(c['left'])} {c['op']} {side_text(c['right'])}"
    return f"{key[1]} being one of its enum's values"


def strings_clash(atoms, case, name, extra_ok=None):
    """Whether the strings a case lets `name` be are none at all — or, with `extra_ok`, all
    among those. Needs at least one list the case says the value is in."""
    yes = [a["values"] for i in case for a in [atoms[i]] if a.get("str") == name and a.get("in")]
    no = [a["values"] for i in case for a in [atoms[i]] if a.get("str") == name and not a.get("in")]
    if not yes:
        return False
    for v in yes[0]:
        out = any(v not in y for y in yes) or any(v in n for n in no)
        if not out and not (extra_ok is not None and v in extra_ok):
            return False
    return True


def contract_proof(atoms, case, door, proof):
    """Whether one proof shows one case keeps one door. None when it does, or what is wrong."""
    if "clash" in proof:
        n = proof["clash"]
        bools = {atoms[i]["value"] for i in case if atoms[i].get("bool") == n}
        if len(bools) == 2 or strings_clash(atoms, case, n):
            return None
        return f"the values of {n} in this case do not clash"
    if proof.get("within"):
        if not isinstance(door[1], list):
            return "a numeric condition is not kept by the strings of a case"
        if strings_clash(atoms, case, door[0], door[1]):
            return None
        return f"this case lets {door[0]} be a value the enum does not have"
    if "farkas" in proof:
        total, k, strict = {}, Fraction(0), False
        for ref in proof["farkas"]:
            y = num(ref.get("y"))
            if y is None or y < 0:
                return "a multiplier is not a number at least zero"
            if ref.get("door"):
                if isinstance(door[1], list):
                    return "an enum's values are not an inequality"
                terms, fk, fs = door[1]
                terms, fk, fs = {n: -v for n, v in terms.items()}, -fk, not fs
            elif "atom" in ref:
                i, part = ref["atom"], ref.get("part", 0)
                if i not in case:
                    return f"it names atom {i}, which is not in this case"
                a = atoms[i]
                if "num" not in a or a.get("rel") not in ("le", "lt", "eq"):
                    return f"atom {i} is not an inequality"
                terms = {n: num(v) for n, v in a["num"].items()}
                fk = num(a["k"])
                if a["rel"] == "eq" and part == 1:
                    terms, fk = {n: -v for n, v in terms.items()}, -fk
                elif part != 0:
                    return f"atom {i} has no part {part}"
                fs = a["rel"] == "lt"
            else:
                return "it names an inequality from nowhere this program knows"
            for n, v in terms.items():
                total[n] = total.get(n, Fraction(0)) + y * v
            k += y * fk
            strict = strict or (fs and y > 0)
        if any(v != 0 for v in total.values()):
            return "the names do not cancel"
        if k > 0 or (k == 0 and strict):
            return None
        return "what is left is not false"
    return "a proof of a kind this program does not know"


def check_contract(cert, k):
    """Re-check one contract's section. Returns a one-line summary; raises Bad on a claim that
    fails."""
    name = k["shape"]
    atoms, cases = k["atoms"], k.get("cases")
    doors = contract_doors(cert, k)
    if cases is None:
        STATED.append(f"contract {name}: its condition opens into too many cases to show anything about")
        return f"contract {name}: too many cases to open (not re-checked)"
    said = {}
    for d in k["doors"]:
        key = ("range", d["range"], d["hi"]) if "range" in d else ("constraint", d["constraint"]) if "constraint" in d else ("member", d.get("member"))
        said[key] = d.get("proofs")
    shown = 0
    for key, door in doors.items():
        proofs = said.get(key)
        what = door_text(cert, key)
        if proofs is None:
            STATED.append(f"contract {name}: {what} is not shown to hold")
            continue
        if len(proofs) != len(cases):
            raise Bad(f"contract {name}: {what} has {len(proofs)} proofs for {len(cases)} cases")
        for ci, (case, proof) in enumerate(zip(cases, proofs)):
            why = contract_proof(atoms, case, (key[1], door), proof)
            if why is not None:
                raise Bad(f"contract {name}: {what}, case {ci + 1}: {why}")
        shown += 1
    if k.get("unread"):
        STATED.append(f"contract {name}: some of its rules could not be read and were taken as true")
    return f"contract {name}: {shown} of {len(doors)} things the door asks hold in all {len(cases)} cases"


def axis_of(u, column):
    """Where a table cuts a column, or None."""
    return next((i for i, a in enumerate(u["axes"]) if a["column"] == column), None)


def numeric_axis(a):
    """Whether the coordinates of an axis stand for numbers."""
    return any(b is not None for b in (a.get("bounds") or []))


def numbered(t, name):
    """Whether this table numbers a value for the name: a numeric axis of its, or a name of its
    linear model (`linear.extra`). The proved checker numbers the values the same way."""
    if any(a["column"] == name and numeric_axis(a) for a in t["axes"]):
        return True
    return name in (t.get("linear") or {}).get("extra", [])


def writers(t, column, value):
    """The table that decides a column, and the rows of it that write the value — counted here,
    from that table's own `produces`. A row that writes something this program cannot read
    leaves the rows uncounted, and the leaf that rests on them is refused."""
    u, oi = decider(t.get("_tables") or [], column)
    if u is None:
        raise Bad(f"{t['table']}: a leaf rests on the rows that write {column} = {value}, and no table here decides {column}")
    rows = []
    for r in u["rows"]:
        p = r.get("produces")
        if not isinstance(p, list) or oi >= len(p) or p[oi] is None:
            raise Bad(f"{t['table']}: row {r['row']} of {u['table']} writes something this program cannot read, "
                      f"so the rows that write {column} = {value} cannot be counted")
        if p[oi] == value:
            rows.append(r)
    return u, rows


def row_out_holds(t, cert, u, row, proof, box):
    """Why a row of a table above fires nowhere in the box: on a column of words both tables
    cut, the box takes none of the words the row lets in (`clash`); or the linear model, the
    box's ends and the row's own ends add up to a contradiction (`farkas`). None when the reason
    holds, or what is wrong."""
    if not isinstance(proof, dict):
        return "a reason this program cannot read"
    if "clash" in proof:
        aj = proof["clash"]
        if not isinstance(aj, int) or not 0 <= aj < len(t["axes"]):
            return f"it names axis {aj}, which does not exist"
        a = t["axes"][aj]
        ua = axis_of(u, a["column"])
        if ua is None:
            return f"{u['table']} does not cut {a['column']}"
        if numeric_axis(a) or numeric_axis(u["axes"][ua]):
            return f"{a['column']} is a number, which a clash of words does not settle"
        mine = {a["coords"][c] for c in box[aj]}
        lets = {u["axes"][ua]["coords"][x] for x in row["accepts"][ua]}
        if mine & lets:
            return f"the box and row {row['row']} of {u['table']} both take {sorted(mine & lets)[0]} on {a['column']}"
        return None
    if "farkas" in proof:
        return farkas_holds(t, cert, proof["farkas"], box, (u, row))
    return "a reason of a kind this program does not know"


def above_keys(node, path=(), out=None):
    """The (axis, coordinate) pairs the cover rests on the rows of a table above for."""
    out = [] if out is None else out
    if "split" in node:
        for c, kid in enumerate(node["split"]):
            above_keys(kid, path + (c,), out)
    elif "above_rows" in node:
        ai = node["above_rows"].get("axis")
        if isinstance(ai, int) and ai < len(path) and (ai, path[ai]) not in out:
            out.append((ai, path[ai]))
    return out


def coord_holds(u, ua, x, v):
    """Whether a value lies in one coordinate of a table's axis, read exactly: the one value of
    a point, strictly between the ends of an interval."""
    lo, hi = bound_at(u, ua, x)
    if lo is not None and hi is not None and lo == hi:
        return v == lo
    return (lo is None or lo < v) and (hi is None or v < hi)


def above_admits(t, at, values, extra):
    """Where the cover rests on the rows of a table above for a value a point takes, one of
    those rows has to fire on the values behind the point (§15.195): each numeric column it cuts
    that this table numbers a value for holds a value inside one of the row's own coordinates,
    and each column of words both tables cut holds a word the row lets in. None when it does,
    or why not."""
    if t.get("cover") is None:
        return None
    cols = [a["column"] for a in t["axes"]]
    vals = {n: num(v) for n, v in zip(cols, values or []) if v is not None}
    vals.update({n: num(v) for n, v in zip((t.get("linear") or {}).get("extra", []), extra or []) if v is not None})
    for ai, ci in above_keys(t["cover"]):
        if at[ai] != ci:
            continue
        column, value = cols[ai], t["axes"][ai]["coords"][ci]
        u, ws = writers(t, column, value)
        for r in ws:
            fires = True
            for ua, ax in enumerate(u["axes"]):
                n = ax["column"]
                if numeric_axis(ax):
                    if not numbered(t, n):
                        continue
                    if n not in vals or not any(coord_holds(u, ua, x, vals[n]) for x in r["accepts"][ua]):
                        fires = False
                        break
                else:
                    aj = axis_of(t, n)
                    if aj is None or numeric_axis(t["axes"][aj]):
                        continue
                    if t["axes"][aj]["coords"][at[aj]] not in {ax["coords"][x] for x in r["accepts"][ua]}:
                        fires = False
                        break
            if fires:
                break
        else:
            return f"no row of {u['table']} that writes {column} = {value} fires on the values behind it"
    return None


def check_cover(t):
    """Completeness: the cover has to tile the space, and every leaf has to hold."""
    axes, rows = t["axes"], {r["row"]: r for r in t["rows"]}
    if t.get("cover") is None:
        raise Bad(f"{t['table']}: no cover is stated, so completeness is not shown")
    seen = {"rows": 0, "constraint": 0, "derived": 0, "define": 0, "truth": 0, "above": 0, "above_rows": 0,
            "points": 0, "model": 0, "days": 0}

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
                raise Bad(f"{t['table']}: {side_text(k['left'])} {k['op']} {side_text(k['right'])} can hold here, so the box is not impossible")
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
        if "define_axis" in node:
            # The axis of a `define` of a number, whose coordinate lies outside what the define's
            # own expression reaches — the interval worked out here, with an interval coordinate's
            # ends left out (§15.195).
            ai = node["define_axis"]
            if not isinstance(ai, int) or not 0 <= ai < len(axes):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which does not exist")
            if ai >= len(path):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which this box has not fixed")
            out = derived_rules_out(t, ai, path)
            if out is None:
                raise Bad(f"{t['table']}: {axes[ai]['column']} is called out of reach, but no expression for it is in the certificate")
            if not out:
                raise Bad(f"{t['table']}: {axes[ai]['column']} can reach this box, so it is not impossible")
            seen["define"] += 1
            return
        if "truth_axis" in node:
            # The axis of a boolean `define` at the truth value it never takes: its comparison,
            # read over the intervals of its two sides worked out here (§15.196).
            ai = node["truth_axis"]
            if not isinstance(ai, int) or not 0 <= ai < len(axes):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which does not exist")
            if ai >= len(path):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which this box has not fixed")
            if not truth_rules_out(t, ai, path):
                raise Bad(f"{t['table']}: {axes[ai]['column']} can be {axes[ai]['coords'][path[ai]]} here, so the box is not impossible")
            seen["truth"] += 1
            return
        if "above_rows" in node:
            # A value decided above that only some rows of that table write, none of which fires
            # in the box (§15.195). The rows are counted here from that table's `produces`, and
            # each comes with a reason that is checked: a clash of words, or multipliers.
            a = node["above_rows"]
            ai = a.get("axis") if isinstance(a, dict) else None
            if not isinstance(ai, int) or not 0 <= ai < len(axes) or ai >= len(path):
                raise Bad(f"{t['table']}: a leaf rests on a table above for axis {ai}, which this box has not fixed")
            column, value = axes[ai]["column"], axes[ai]["coords"][path[ai]]
            if a.get("column") != column or a.get("value") != value:
                raise Bad(f"{t['table']}: a leaf speaks of {a.get('column')} = {a.get('value')}, and the box holds {column} = {value}")
            u, ws = writers(t, column, value)
            if not ws:
                raise Bad(f"{t['table']}: no row of {u['table']} writes {column} = {value}, which is a fact, not this leaf")
            given = {}
            for r in a.get("rows") or []:
                n = r.get("row") if isinstance(r, dict) else None
                if not isinstance(n, int) or n in given:
                    raise Bad(f"{t['table']}: a leaf names a row of {u['table']} it cannot, or twice")
                given[n] = r
            want = sorted(r["row"] for r in ws)
            if sorted(given) != want:
                raise Bad(f"{t['table']}: the rows of {u['table']} that write {column} = {value} are {want}, "
                          f"and the leaf gives reasons for {sorted(given)}")
            box = [[c] for c in path] + [list(range(len(x["coords"]))) for x in axes[len(path):]]
            for r in ws:
                why = row_out_holds(t, t["_cert"], u, r, given[r["row"]], box)
                if why is not None:
                    raise Bad(f"{t['table']}: row {r['row']} of {u['table']} is said to fire nowhere in this box, and {why}")
            seen["above_rows"] += 1
            return
        if "days_axis" in node:
            ai = node["days_axis"]
            if not isinstance(ai, int) or not 0 <= ai < len(axes):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which does not exist")
            out = day_rules_out(t, ai, path)
            if out is None:
                raise Bad(f"{t['table']}: {axes[ai]['column']} is said to miss koyomi's days here, and the certificate states no days for it")
            if not out:
                raise Bad(f"{t['table']}: a day {axes[ai]['column']} takes lies in this box, so it is not impossible")
            seen["days"] += 1
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


def optional_input(t, a):
    """Whether an axis is an input the rule declares optional (`T?`): its type, as the
    certificate states it, ends in `?`."""
    types = (t.get("_cert") or {}).get("types", {})
    return a.get("kind") == "input" and str(types.get(a["column"], "")).endswith("?")


def enum_order(t, axis):
    """The values of the ordered enum an axis is of, in their order (rulec's §15.210). An axis
    of any other type has none, and a cell that compares on one is not a cell of the rule."""
    cert = t.get("_cert") or {}
    ty = str((cert.get("types") or {}).get(axis["column"], "")).rstrip("?")
    if ty not in (cert.get("ordered") or []):
        raise Bad(f"{t['table']}: {axis['column']} is compared by order, and {ty or 'its type'} has none")
    return list((cert.get("enums") or {}).get(ty) or [])


def check_enum_axis(t, ai):
    """An input of an enum runs over every value of the enum and no other — or over the values
    its range lets in, for an ordered one that declares one (rulec's §15.210) — with the absent
    value first where it is optional. A value quietly left off would be a value no row was ever
    asked to cover."""
    a = t["axes"][ai]
    cert = t.get("_cert") or {}
    ty = str((cert.get("types") or {}).get(a["column"], ""))
    enums = cert.get("enums") or {}
    if a.get("kind") != "input" or ty.rstrip("?") not in enums:
        return
    want = [v for v in enums[ty.rstrip("?")] if a["column"] not in (cert.get("enum_ranges") or {})
            or v in cert["enum_ranges"][a["column"]]]
    if ty.endswith("?"):
        want = ["none"] + want
    if list(a.get("coords") or []) != want:
        raise Bad(f"{t['table']}: {a['column']}'s axis is not the values its enum lets in: "
                  f"{a.get('coords')} where {want} was declared")


def check_axis(t, ai):
    """A numeric axis has to tile: its coordinates run from the declared range's low end to
    its high end, each touching the next or one grid step past it, with nothing between.
    §6.2 builds it that way; without this check a coordinate could be quietly removed and
    the gap under it would never be covered by anything (§15.99).

    An optional input's axis starts with the absent value, `none`, which stands for no number
    at all; the values that are there tile the rest (§15.201). A certificate that leaves the
    absent value out has covered less than the inputs the rule takes.

    The axis of what a walk leaves behind — a `count`, a `sum`, a `max`, a `min`, a fold's answer
    — runs over the range `ranges` states for it, as an input's runs over its declared range: the
    table that reads it is complete over that range and no further, and the range is the one the
    sections on `extremes` and `fold_answers` hold to what the walk can answer (§15.236). An open
    value runs over the range the rule declares for it, which `opens` holds to what its terms come to
    together (§15.227). Any other column is cut open at both ends."""
    a = t["axes"][ai]
    bs = a.get("bounds") or []
    if optional_input(t, a):
        coords = a.get("coords") or []
        if not coords or coords[0] != "none" or (bs and bs[0] is not None):
            raise Bad(f"{t['table']}: {a['column']} is optional, and its axis has no coordinate for the absent value")
        bs = bs[1:]
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
    # Two coordinates side by side: two points a step apart, or a point and an interval that begins
    # or ends at it. Two intervals that meet leave the value they meet at out of both, and one point
    # written twice puts a value on two coordinates (`axisTilesB` in the Lean program).
    for k in range(len(vals) - 1):
        (a1, b1), (a2, b2) = vals[k], vals[k + 1]
        p1 = a1 is not None and a1 == b1
        p2 = a2 is not None and a2 == b2
        if p1 and p2:
            ok = a2 == b1 + step
        elif p1:
            ok = a2 == b1
        elif p2:
            ok = b1 is not None and a2 == b1
        else:
            ok = False
        if not ok:
            raise Bad(f"{t['table']}: {a['column']} has a gap between {b1} and {a2}, or a value on two coordinates")
    if a.get("kind") not in ("input", "walk", "open") and (vals[0][0] is not None or vals[-1][1] is not None):
        # A column the rule computes, or one a table above decides, is cut open at both ends: what
        # it can take is the whole line, and an axis that stopped short would leave the rest out.
        raise Bad(f"{t['table']}: {a['column']} does not run the whole of the values it can take")
    if a.get("kind") in ("input", "walk", "open"):
        r = t.get("_ranges", {}).get(a["column"])
        if r is not None:
            lo, hi = num(r[0]), num(r[1])
            said = "the rule declares" if a.get("kind") in ("input", "open") else "its range is"
            if vals[0][0] != lo or vals[-1][1] != hi:
                raise Bad(f"{t['table']}: {a['column']} runs from {vals[0][0]} to "
                          f"{vals[-1][1]}, and {said} {lo} to {hi}")
            # A coordinate that is an interval leaves its ends out, so an end of the declared
            # range is only covered by a point on it. An axis whose last point was taken away
            # still reached the end with the interval before it, and the value at the end was
            # covered by nothing (§15.201).
            if (lo is not None and vals[0][1] != lo) or (hi is not None and vals[-1][0] != hi):
                raise Bad(f"{t['table']}: {a['column']} does not end on the ends of the range the rule declares, "
                          f"so a value at an end is in no coordinate")
    # "One step past it, with nothing between" is only so on the grid the column's values
    # really sit on (§15.190). A column of tenths of a pound cut every whole pound has values
    # between two coordinates, and the gap under them was covered by nothing.
    g = column_grid(t.get("_cert") or {}, a["column"])
    if g is None:
        STATED.append(f"{t['table']}: the step {a['column']} is cut at, which no scale here bears out")
        return 1
    if (g / step).denominator != 1:
        raise Bad(f"{t['table']}: {a['column']} is cut every {step}, and its values sit on a step of {g}, "
                  f"so a value can fall between two coordinates")
    for lo, hi in vals:
        for e in (lo, hi):
            if e is not None and (e / step).denominator != 1:
                raise Bad(f"{t['table']}: {a['column']} has a coordinate ending at {e}, off its step of {step}")
    return 1


ROUNDINGS = ("up", "down", "half_up", "half_even", "half_down")


def share(a, b):
    """The largest step two steps are both whole multiples of: a tenth and a whole unit share a
    tenth. Zero, the step of a value that is always zero, divides nothing and is left out."""
    if a == 0:
        return b
    if b == 0:
        return a
    return Fraction(math.gcd(a.numerator * b.denominator, b.numerator * a.denominator),
                    a.denominator * b.denominator)


def expr_grid(e, grid_of):
    """The step every value of an expression is a multiple of, from its leaves up, as `rulec`
    works it out (§15.190): a name is on its own step and a literal on its value; a sum or a
    difference on the step both sides share, a product on the product of the two, a quotient
    by a whole constant on its left side's over the constant; `min` and `max` on the step both
    sides share, a rounding on its grid, `allocate` on whole units. None for anything else."""
    if "name" in e:
        return grid_of(e["name"])
    if "num" in e:
        v = num(e.get("value"))
        return None if v is None else abs(v)
    op = e.get("op")
    if op in ("+", "-", "*"):
        a, b = expr_grid(e["l"], grid_of), expr_grid(e["r"], grid_of)
        if a is None or b is None:
            return None
        return a * b if op == "*" else share(a, b)
    if op == "/":
        a = expr_grid(e["l"], grid_of)
        k = num(e["r"].get("value")) if "num" in e["r"] else None
        if a is None or k is None or k.denominator != 1 or k <= 0:
            return None
        return a / k
    if "call" in e:
        args = e.get("args", [])
        if e["call"] in ("min", "max") and len(args) == 2:
            a, b = expr_grid(args[0], grid_of), expr_grid(args[1], grid_of)
            return None if a is None or b is None else share(a, b)
        if e["call"] in ROUNDINGS and len(args) == 2 and "num" in args[1]:
            v = num(args[1].get("value"))
            return None if v is None else abs(v)
        if e["call"] == "allocate":
            return Fraction(1)
    return None


def column_grid(cert, name, seen=()):
    """The step every value of a column is a multiple of, from the document's own account: a
    value's expression read from its leaves up, and otherwise — and for a value whose
    expression gives no step — the scale the name is stored at (`scales`). None when neither
    says, which a certificate from before §15.190 does not."""
    if name in seen:
        return None
    grid_of = lambda n: column_grid(cert, n, seen + (name,))
    v = next((x for x in cert.get("values", []) if x.get("name") == name), None)
    if v is not None and v.get("expr") is not None:
        g = expr_grid(v["expr"], grid_of)
        if g is not None and g != 0:
            return g
    # What a walk answers (§15.219, §15.220): a fold's answer is the value of one of the
    # expressions it answers with, and a `max` or a `min` a value of its column or its `empty`
    # value, so each sits on the step those share.
    fa = next((x for x in cert.get("fold_answers") or [] if x.get("name") == name), None)
    ex = next((x for x in cert.get("extremes") or [] if x.get("name") == name), None)
    g = None
    if fa is not None:
        gs = [expr_grid(e, grid_of) for e in fa.get("answers", [])]
        if gs and all(x is not None for x in gs):
            g = Fraction(0)
            for x in gs:
                g = share(g, x)
    elif ex is not None:
        # An ordered enum's `empty` is a word, and its column has no step.
        c = grid_of(ex.get("of"))
        if c is not None and isinstance(ex.get("empty"), str) and re.fullmatch(r"-?\d+(/\d+)?", ex["empty"]):
            g = share(c, abs(num(ex["empty"])))
    if g is not None and g != 0:
        return g
    s = (cert.get("scales") or {}).get(name)
    return Fraction(1, s) if isinstance(s, int) and s > 0 else None


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
        elif kind in ("in", "not_in"):
            # A set of numbers: each value is a point of the axis, so a coordinate is taken
            # when it is one of those points (or, for `not`, when it is none of them).
            tests = [("=", num(v)) for v in cell["values"]]
            if any(v is None for _, v in tests):
                return None
            if not axis_splits(axis, tests):
                raise Bad(
                    f"{axis['column']}: the cell names a value that falls inside a coordinate, "
                    f"so the axis does not stand for what the cell says")
            hit = []
            for i in range(len(coords)):
                b = bounds[i]
                # The absent value of an optional input is none of the values (§15.201).
                if b is None and coords[i] == "none":
                    continue
                if b is None:
                    return None
                if any(coord_admits((num(b[0]), num(b[1])), op, v) for op, v in tests):
                    hit.append(i)
            out.append(hit if kind == "in" else [i for i in range(len(coords)) if i not in set(hit)])
        elif kind == "rank":
            # A comparison on an ordered enum (rulec's §15.210): the values whose place in the
            # order the certificate gives the enum passes every comparison. The absent value of
            # an optional input is no value of the enum.
            order = enum_order(t, axis)
            tests = [(x["op"], x["value"]) for x in cell["tests"]]
            if any(v not in order for _, v in tests):
                raise Bad(f"{t['table']}: {axis['column']} is compared against a value its enum does not have")
            at = {w: i for i, w in enumerate(order)}
            ops = {"<": lambda a, b: a < b, "<=": lambda a, b: a <= b, ">": lambda a, b: a > b, ">=": lambda a, b: a >= b}
            out.append([i for i, c in enumerate(coords) if c in at and all(ops[op](at[c], at[v]) for op, v in tests)])
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
                # A comparison holds only of a value that is there (§15.201).
                if b is None and coords[i] == "none":
                    continue
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
            # An enum or a flag: the sieve has nothing to say about it, but where it is a
            # boolean `define` that always takes one value (§15.196).
            if truth_rules_out(t, ai, at):
                return f"{axes[ai]['column']} is never {axes[ai]['coords'][c]}"
            continue
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
        if reach is not None and axes[ai]["kind"] != "input" and not (reach[0] <= v <= reach[1]):
            return f"{axes[ai]['column']} = {v} is outside what its own expression reaches"
        days = days_of_axis(t, ai)
        if days is not None and v not in days:
            return f"{axes[ai]['column']} = {v} is not one of the days koyomi computes for it"
    for k in t["constraints"]:
        names = side(k["left"]) + side(k["right"])
        if any(n not in col for n in names):
            continue
        got = {n: vs[col.index(n)] for n in names}
        if any(v is None for v in got.values()):
            continue
        x = sum((num(got[n]) for n in side(k["left"])), Fraction(0))
        y = sum((num(got[n]) for n in side(k["right"])), Fraction(0))
        ok = {"<=": x <= y, "<": x < y, ">=": x >= y, ">": x > y}[k["op"]]
        if not ok:
            return f"{side_text(k['left'])} {k['op']} {side_text(k['right'])} does not hold at {x}, {y}"
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
        check_enum_axis(t, ai)
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

    # (0) The box each row states is the box its own cells describe — less the coordinates of a
    # column decided above whose values no table above writes (rulec's §15.253). rulec reads every
    # box without them, since no input brings the column there; each one left out has to be a
    # `never` fact of this table's, and (3) earns every such fact back from the rows of the
    # table that decides the column. A coordinate left out with no fact is a box narrower than its
    # cell, and is refused like a wider one.
    never = {(f.get("axis"), f.get("coord")) for f in (t.get("above") or {}).get("never", [])}
    from_cells = narrowed = 0
    for r in t["rows"]:
        if "tests" not in r:
            continue
        got = box_from_cells(t, r, GROUPS)
        if got is None:
            continue
        left_out = False
        for ai, coords in enumerate(got):
            want = [c for c in coords if (ai, c) not in never]
            left_out = left_out or len(want) != len(coords)
            if sorted(want) != sorted(r["accepts"][ai]):
                without = " once the values the tables above never write are left out" if len(want) != len(coords) else ""
                raise Bad(
                    f"{name}: row {r['row']}'s box on {axes[ai]['column']} is not what its cell "
                    f"`{r['cells'][ai]}` describes{without}"
                )
        from_cells += 1
        narrowed += left_out

    # (1) No two rows of a `unique` table meet.
    pairs = model_pairs = days_pairs = above_pairs = 0
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
        # The pairs koyomi's days part (§15.174): on an axis of days, every coordinate both
        # rows take holds none of them, so no point the rule is asked about is in both.
        for d in t.get("days_apart", []):
            a, b, ai = d["a"], d["b"], d["axis"]
            if a not in rows or b not in rows:
                raise Bad(f"{name}: rows {a} and {b} are not both in the table")
            if not 0 <= ai < len(axes) or days_of_axis(t, ai) is None:
                raise Bad(f"{name}: rows {a} and {b} are said to part on koyomi's days of axis {ai}, which has none")
            shared = sorted(set(rows[a]["accepts"][ai]) & set(rows[b]["accepts"][ai]))
            held = [c for c in shared if not day_rules_out(t, ai, [0] * ai + [c])]
            if held:
                raise Bad(f"{name}: rows {a} and {b} are said to part on koyomi's days, and both take a day of them on {axes[ai]['column']}")
            told[(a, b)] = None
            days_pairs += 1
        # The pairs the rows of a table above part (§15.196): for each value both rows take on
        # the column it decides, every row of it that writes the value — counted here — fires
        # nowhere in what both rows take, for a reason that is checked.
        for d in t.get("above_apart", []):
            a, b, ai = d.get("a"), d.get("b"), d.get("axis")
            if a not in rows or b not in rows:
                raise Bad(f"{name}: rows {a} and {b} are not both in the table")
            if not isinstance(ai, int) or not 0 <= ai < len(axes):
                raise Bad(f"{name}: rows {a} and {b} name axis {ai}, which does not exist")
            pb = [sorted(set(rows[a]["accepts"][x]) & set(rows[b]["accepts"][x])) for x in range(len(axes))]
            at = {}
            for x in d.get("at") or []:
                c = x.get("coord") if isinstance(x, dict) else None
                if not isinstance(c, int) or c in at:
                    raise Bad(f"{name}: rows {a} and {b} give a value of {axes[ai]['column']} it cannot, or twice")
                at[c] = x.get("above_rows")
            if sorted(at) != pb[ai]:
                raise Bad(f"{name}: rows {a} and {b} both take {[axes[ai]['coords'][c] for c in pb[ai]]} on {axes[ai]['column']}, "
                          f"and reasons are given for {[axes[ai]['coords'][c] for c in sorted(at) if c < len(axes[ai]['coords'])]}")
            column = axes[ai]["column"]
            for c in pb[ai]:
                ar, value = at[c], axes[ai]["coords"][c]
                if not isinstance(ar, dict) or ar.get("axis") != ai or ar.get("column") != column or ar.get("value") != value:
                    raise Bad(f"{name}: rows {a} and {b}: the reason for {column} = {value} speaks of something else")
                u, ws = writers(t, column, value)
                if not ws:
                    raise Bad(f"{name}: no row of {u['table']} writes {column} = {value}, which is a fact, not a reason")
                given = {}
                for r in ar.get("rows") or []:
                    n = r.get("row") if isinstance(r, dict) else None
                    if not isinstance(n, int) or n in given:
                        raise Bad(f"{name}: rows {a} and {b}: a row of {u['table']} is named that cannot be, or twice")
                    given[n] = r
                want = sorted(r["row"] for r in ws)
                if sorted(given) != want:
                    raise Bad(f"{name}: the rows of {u['table']} that write {column} = {value} are {want}, "
                              f"and rows {a} and {b} give reasons for {sorted(given)}")
                box = [list(x) for x in pb]
                box[ai] = [c]
                for r in ws:
                    why = row_out_holds(t, t["_cert"], u, r, given[r["row"]], box)
                    if why is not None:
                        raise Bad(f"{name}: rows {a} and {b} are said to part on what {u['table']} writes, and row {r['row']} of it {why}")
            told[(a, b)] = None
            above_pairs += 1
        undecided = {(u["a"], u["b"]) for u in t["undecided"]}
        numbers = sorted(rows)
        for i, a in enumerate(numbers):
            for b in numbers[i + 1:]:
                if (a, b) not in told and (a, b) not in undecided:
                    raise Bad(f"{name}: rows {a} and {b} are neither proved apart nor listed as undecided")
    elif t["disjoint"] or t.get("refuted") or t.get("days_apart") or t.get("above_apart"):
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
        if why is None:
            why = above_admits(t, at, r.get("at_values"), r.get("extra_values"))
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
    # Rows the sieve rules out entirely. E102 reads the sieve only where a derive's reach takes
    # part (DESIGN §15.189), so `check` passes the rest; the certificate names them and this
    # program repeats the name rather than proving it.
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
    # leaf that rests on it rests on something already checked. The words each row is said to
    # write are values of their columns before anything reads them (rulec's §15.261).
    check_words(t)
    check_above(t)
    seen = check_cover(t)
    if True:
        cover_note = f", {seen['rows']} boxes covered"
        for k, word in (("constraint", "by a constraint"), ("derived", "out of a derive's reach"),
                        ("define", "out of a define's reach"),
                        ("truth", "at a truth value a define never takes"),
                        ("above", "for the tables above"),
                        ("above_rows", "by the rows of a table above"), ("points", "point by point"),
                        ("model", "by the linear model"), ("days", "outside koyomi's days")):
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
    if narrowed:
        boxes += f" ({narrowed} without the values the tables above never write)"
    apart = f" + {model_pairs} apart on the linear model" if model_pairs else ""
    if days_pairs:
        apart += f" + {days_pairs} apart on koyomi's days"
    if above_pairs:
        apart += f" + {above_pairs} apart on the rows of a table above"
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
    # `not: a, b` and `not a, b` alike (§15.196): the colon is optional, and a word that only
    # begins with the letters (`notice`) is a literal.
    if s.startswith("not") and (s[3:4] == ":" or s[3:4].isspace()):
        rest = s[3:].lstrip()
        return ("not", split_words(rest[1:] if rest.startswith(":") else rest))
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
    """The byte ranges a table row's `|` separators cut the line into, each trimmed, read as
    rulec's lexer reads the line: a `|` inside a string (`"…"`, which has no escapes) is the
    string's, and a `#` outside one starts the comment, which runs to the end of the line, so a
    `|` after it is the comment's. The text after the last `|` is a comment or a citation, not a
    cell, so it is not one of them."""
    bars = []
    in_string = False
    for i, b in enumerate(line):
        if b == ord('"'):
            in_string = not in_string
        elif not in_string and b == ord("#"):
            break
        elif not in_string and b == ord("|"):
            bars.append(i)
    out = []
    for k in range(len(bars) - 1):
        a, b = bars[k] + 1, bars[k + 1]
        seg = line[a:b]
        out.append((a + len(seg) - len(seg.lstrip()), b - (len(seg) - len(seg.rstrip()))))
    return out


def row_file(r):
    """The file a row is written in, from the rule's directory, when it is not the rule's own: a
    row a contribution adds (rulec's §15.231). `""` for the rule's own file."""
    f = r.get("file")
    return f.get("path", "") if isinstance(f, dict) else ""


def contribution_lines(cert, path):
    """The lines of every file a row of the certificate is written in besides the rule's own, read
    from the rule's directory and held to the SHA-256 the row names (rulec's §15.231)."""
    out = {}
    base = os.path.dirname(path)
    for t in cert["tables"]:
        for r in t["rows"]:
            f = r.get("file")
            if not isinstance(f, dict):
                continue
            rel = f.get("path", "")
            there = os.path.normpath(os.path.join(base, rel))
            try:
                with open(there, "rb") as fh:
                    data = fh.read()
            except OSError:
                raise Bad(f"{t['table']}: row {r['row']} is written in {rel}, which cannot be read from {base or '.'}")
            if hashlib.sha256(data).hexdigest() != f.get("sha256"):
                raise Bad(f"{t['table']}: row {r['row']} is written in {rel}, and that file is another text than the certificate's")
            out[rel] = data.split(b"\n")
    return out


def answer_words(t, r):
    """What each answer cell of a row says, as the certificate reads it: the word it writes
    (`produces`), or the name of the column of its own table it passes on (`passes`, rulec's
    §15.261), None where it says neither."""
    words = list(r.get("produces") or [])
    for j, a in enumerate(r.get("passes") or []):
        if a is None:
            continue
        if not isinstance(a, int) or isinstance(a, bool) or not 0 <= a < len(t["axes"]):
            raise Bad(f"{t['table']}: row {r['row']}, answer {j + 1}: it passes on axis {a}, which does not exist")
        while len(words) <= j:
            words.append(None)
        if words[j] is not None:
            raise Bad(f"{t['table']}: row {r['row']}, answer {j + 1}: the certificate reads both a word and a column there")
        words[j] = t["axes"][a]["column"]
    return words


def check_cells(cert, path):
    """The table stated here is the table in the file. Returns (cells read back, literals
    whose number could not be pinned, rows written in an applied rule)."""
    with open(path, "rb") as fh:
        own_lines = fh.read().split(b"\n")
    # A row a contribution adds is written in its file (rulec's §15.231), read from there.
    others = contribution_lines(cert, path)
    read = loose = apart = 0
    for t in cert["tables"]:
        axes = t["axes"]
        # Rows written in one table have a cell in the same columns and in no others, and
        # they are written one under another with no other table's rows between them — in the
        # one file they are written in.
        shape, lines_of, seen_axis, at_line, written = {}, {}, [False] * len(axes), {}, {}
        for r in t["rows"]:
            src = r.get("source")
            o = (row_file(r), r.get("origin", ""))
            # A table is either written in this file or brought in by an `apply`; its rows
            # cannot differ about that. Without this one row could hide behind `null` and
            # never be read back (§15.99).
            was = written.setdefault(o, src is not None)
            if was != (src is not None):
                raise Bad(f"{t['table']}: some rows of `{o[1]}` are written in this file and "
                          f"some are not, which no one table is")
            if src is None:
                continue
            here = tuple(sp is None for sp in src)
            was = shape.setdefault(o, here)
            if was != here:
                raise Bad(f"{t['table']}: rows of `{o[1]}` disagree about which columns they "
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
                raise Bad(f"{t['table']}: row {r['row']} of `{o[1]}` is written at line "
                          f"{ln}, at or above the row before it")
            at_line[o] = ln
            lo, hi = lines_of.get(o, (ln, ln))
            lines_of[o] = (min(lo, ln), max(hi, ln))
            for ai, sp in enumerate(src):
                if sp is not None:
                    seen_axis[ai] = True
        for a, (alo, ahi) in lines_of.items():
            for b, (blo, bhi) in lines_of.items():
                if a[0] == b[0] and a < b and alo <= bhi and blo <= ahi:
                    raise Bad(f"{t['table']}: rows of `{a[1]}` and `{b[1]}` are written among "
                              f"each other, which no two tables are")
        if lines_of and not all(seen_axis):
            # A column of a table an `apply` brought in has its cells in that rule's file, where
            # its own certificate reads them back (§15.196): only a column no row has a cell in,
            # here or there, is one no cell is written for.
            applied = [any(r.get("source") is None and i < len(r.get("tests") or []) and r["tests"][i].get("cell") != "any"
                           for r in t["rows"]) for i in range(len(axes))]
            missing = [axes[i]["column"] for i, v in enumerate(seen_axis) if not v and not applied[i]]
            if missing:
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
            lines = others.get(row_file(r), own_lines) if row_file(r) else own_lines
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
            # What the row writes into a column of words is the cell in the file too (§15.233): the
            # answer cell of that column says the word the certificate reads — what a machine moves
            # on, what a `once` line counts, what an `ordered by` table answers, and what a table
            # above writes. Only for a table written in one place, whose answer cells stand in the
            # order it decides its columns (a set of tables that decide one column does not).
            if bars and here and len({x.get("origin", "") for x in t["rows"]}) == 1:
                for j, w in enumerate(answer_words(t, r)):
                    if w is None:
                        continue
                    a, b = bars[len(got) + j] if len(got) + j < len(bars) else (0, 0)
                    said = line[a:b].decode("utf-8", "replace")
                    if said != w:
                        raise Bad(f"{t['table']}: row {r['row']}, answer {j + 1}: the file writes `{said}` there, "
                                  f"and the certificate reads it as `{w}`")
                    read += 1
            # In a table written in more than one place, the certificate names the cell each word is
            # written in (§15.243): one of the row's answer cells on its line, or a `clause`'s `then` line.
            elif len({(row_file(x), x.get("origin", "")) for x in t["rows"]}) > 1:
                answers = r.get("answers") or []
                for j, w in enumerate(answer_words(t, r)):
                    if w is None:
                        continue
                    sp = answers[j] if j < len(answers) else None
                    if not isinstance(sp, dict):
                        STATED.append(f"{t['table']}: row {r['row']}, answer {j + 1}: the word `{w}`, which the certificate names no answer cell for")
                        continue
                    if bars and (sp["line"] != ln or (sp["col"], sp["col"] + sp["len"]) not in bars[len(got):]):
                        raise Bad(f"{t['table']}: row {r['row']}, answer {j + 1}: the cell named is not one of the row's answer cells")
                    wline = line if bars else (lines[sp["line"] - 1] if 0 < sp["line"] <= len(lines) else b"")
                    said = wline[sp["col"]:sp["col"] + sp["len"]].decode("utf-8", "replace")
                    if said != w or sp.get("text") != w:
                        raise Bad(f"{t['table']}: row {r['row']}, answer {j + 1}: the file writes `{said}` there, "
                                  f"and the certificate reads it as `{w}`")
                    read += 1
            # What the row writes into a column that holds a number is the cell in the file
            # (§15.196): the same line, one of the cells after the ones the tests read, and the
            # literal or the name the certificate says it is.
            outs_here = bars[len(got):] if bars else []
            for oi, w in enumerate(r.get("writes") or []):
                if not isinstance(w, dict):
                    continue
                wsp, e = w.get("source"), w.get("expr") or {}
                col = (t.get("decides") or [None] * (oi + 1))[oi] if oi < len(t.get("decides") or []) else None
                where = f"{t['table']}: row {r['row']}, -> {col}"
                if not isinstance(wsp, dict) or not isinstance(wsp.get("line"), int):
                    raise Bad(f"{where}: the row is written in this file, and no cell is named for what it writes")
                if bars:
                    # A table's row: the answer is one of the cells after the ones its tests read,
                    # on the row's own line.
                    if wsp["line"] != ln or (wsp["col"], wsp["col"] + wsp["len"]) not in outs_here:
                        raise Bad(f"{where}: the cell named is not one of the row's answer cells")
                    wline = line
                else:
                    # A `clause`: what it writes stands on its `then` line.
                    wline = lines[wsp["line"] - 1] if 0 < wsp["line"] <= len(lines) else b""
                text = wline[wsp["col"]:wsp["col"] + wsp["len"]].decode("utf-8", "replace")
                if text != wsp.get("text"):
                    raise Bad(f"{where}: the file says `{text}` where the certificate quotes `{wsp.get('text')}`")
                if "name" in e:
                    # A contribution writes its own names without the `<contribution>:` they come
                    # into the rule with (rulec's §15.231).
                    by = (r.get("file") or {}).get("contribution", "") if isinstance(r.get("file"), dict) else ""
                    if text.strip() != e["name"] and not (by and f"{by}:{text.strip()}" == e["name"]):
                        raise Bad(f"{where}: the file writes `{text}`, the certificate reads it as the name {e['name']}")
                elif "num" in e:
                    if text.strip() != e["num"]:
                        raise Bad(f"{where}: the file writes `{text}`, the certificate reads it as `{e['num']}`")
                    loose += output_literal_agrees(where, (cert.get("types") or {}).get(col), text, e.get("value"))
                else:
                    raise Bad(f"{where}: the certificate reads the cell as something an answer cell cannot be")
                read += 1
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
    if shape[0] in ("is", "not") and kind in ("in", "not_in"):
        # A set of numbers: the same members, in the same order, each the value it names.
        if kind != ("in" if shape[0] == "is" else "not_in") or len(shape[1]) != len(st["values"]):
            raise Bad(f"{where}: the values in the file are not the ones the certificate states")
        return sum(literal_agrees(where, axis, w, v) for w, v in zip(shape[1], st["values"]))
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
    if kind == "rank":
        # On an ordered enum (rulec's §15.210): the same comparisons, against the same values.
        if [(op, lit) for op, lit in shape[1]] != [(x["op"], x["value"]) for x in st["tests"]]:
            raise Bad(f"{where}: the comparisons in the file are not the ones the certificate states")
        return 0
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


def output_literal_agrees(where, ty, text, value):
    """1 when the literal's number could not be pinned: a unit this program does not convert.
    Otherwise the number the file writes is the value the certificate states for it, or the
    certificate is reading the cell as something the file does not say (§15.196)."""
    k = lit_key(text)
    if k[0] != "n":
        raise Bad(f"{where}: the file writes `{text}`, which is not a number")
    n, unit = k[1], k[2]
    cur = ty[len("money["):-1].split(",")[0].strip() if isinstance(ty, str) and ty.startswith("money[") else None
    if unit == "%":
        v = n / 100
    elif unit == "" or (cur is not None and unit == cur):
        v = n
    else:
        return 1
    if value is None or v != num(value):
        raise Bad(f"{where}: the file writes `{text}`, the certificate reads it as {value}")
    return 0


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


def check_machine(cert):
    """Re-check a machine's section (§15.148, §15.149). Returns a one-line summary; raises Bad.

    The transitions are rebuilt from the table's own rows: from a state, every row whose box
    takes that state on the state's axis moves the case to the row's target — or leaves it
    where it is. That includes every transition a call can make, so a set that holds the
    initial state and is closed under it holds every state a case can reach, and the claims
    that nothing bad is reached are closures of that kind. That a final state can still be
    reached needs transitions that really happen, so each step of those paths is a point in
    the row's box, at that state, the rule is asked about — checked as a reach point is.

    A case holds its `held` inputs, so where the table reads one as a column a case only takes
    the rows whose box holds its coordinate there. Every claim is checked once per world — each
    combination of those columns' coordinates, every one of them present — with the other rows
    left out. A path to a final state keeps the world's coordinates, which makes it one case's
    when nothing but the input's own column reads it: the table reads no column that is not
    an input, and no constraint names it. Otherwise the paths are not taken as proof.
    """
    m = cert.get("machine")
    if not m.get("table"):
        STATED.append(f"machine: not laid on a table's certificate — {m.get('why', '')}")
        return "machine: not certified here — " + m.get("why", "")
    t = next((x for x in cert["tables"] if x["table"] == m["table"]), None)
    if t is None:
        raise Bad(f"machine: the table {m['table']} has no certificate")
    states, k = m["states"], m["axis"]
    n = len(states)
    axes = t["axes"]
    if k is not None:
        if not 0 <= k < len(axes) or axes[k]["column"] != m["carry"]["input"]:
            raise Bad(f"machine: axis {k} of {m['table']} is not the carried input {m['carry']['input']}")
        if axes[k]["coords"] != states:
            raise Bad("machine: the state's axis is not the states in order")
    rows = {r["row"]: r for r in t["rows"]}
    j = t["decides"].index(m["carry"]["output"]) if m["carry"]["output"] in t["decides"] else None
    if j is None:
        raise Bad(f"machine: {m['table']} does not decide {m['carry']['output']}")
    moves = {}
    for r in m["rows"]:
        row = r["row"]
        if row not in rows:
            raise Bad(f"machine: row {row} is not in {m['table']}")
        produced = rows[row]["produces"][j]
        if "to" in r:
            if not 0 <= r["to"] < n or produced != states[r["to"]]:
                raise Bad(f"machine: row {row} is said to go to {r['to']}, and the table says it writes {produced}")
            moves[row] = r["to"]
        elif r.get("stay"):
            if produced is not None:
                raise Bad(f"machine: row {row} is said to leave the state, and the table says it writes {produced}")
            moves[row] = None
        else:
            raise Bad(f"machine: row {row} has no move")
    if set(moves) != set(rows):
        raise Bad(f"machine: rows {sorted(set(rows) - set(moves))} have no move")
    on = {row: (rows[row]["accepts"][k] if k is not None else list(range(n))) for row in rows}

    # The worlds: every combination of the held columns' coordinates, each once.
    fixed_inputs = m.get("held_inputs", [])
    fixed = m.get("held", [])
    for fk in fixed:
        if not 0 <= fk < len(axes) or axes[fk]["kind"] != "input" or axes[fk]["column"] not in fixed_inputs:
            raise Bad(f"machine: axis {fk} is not a held input's column")
    for fi in fixed_inputs:
        if any(a["column"] == fi for a in axes) and not any(axes[fk]["column"] == fi for fk in fixed):
            raise Bad(f"machine: {fi} is held and a column of {m['table']}, and no world is cut by it")
    want = [[]]
    for fk in fixed:
        want = [w + [c] for w in want for c in range(len(axes[fk]["coords"]))]
    got = [w["at"] for w in m["worlds"]]
    if sorted(got) != sorted(want) or len(got) != len(want):
        raise Bad("machine: the worlds are not every combination of the held columns' coordinates, once each")
    coupled = bool(fixed_inputs) and (
        any(a["kind"] != "input" for a in axes)
        or any(n in fixed_inputs for kc in cert.get("constraints", []) for n in side(kc["left"]) + side(kc["right"]))
    )

    init, finals = m["initial"], set(m["finals"])
    claims = set()
    reached = set()
    weak = 0
    for world in m["worlds"]:
        at_w = world["at"]
        label = "" if not at_w else " where " + ", ".join(f"{axes[fk]['column']} = {axes[fk]['coords'][c]}" for fk, c in zip(fixed, at_w))

        def takes(row):
            return all(c in rows[row]["accepts"][fk] for fk, c in zip(fixed, at_w))

        def step(s):
            return [(row, s if to is None else to) for row, to in moves.items() if s in on[row] and takes(row)]

        reach = set(world["reach"])
        if init not in reach:
            raise Bad(f"machine: the reach set{label} leaves out the initial state")
        for s in reach:
            for row, to in step(s):
                if to not in reach:
                    raise Bad(f"machine: from {states[s]}{label}, row {row} leads to {states[to]}, outside the reach set")
        reached |= reach
        claims.add("reach")
        if world.get("final_certified"):
            for f in finals & reach:
                for row, to in step(f):
                    if to != f:
                        raise Bad(f"machine: row {row} leads out of the final state {states[f]}{label}")
            claims.add("final")
        elif finals & reach:
            STATED.append(f"machine: that no call leaves a final state{label} is not laid on the rows")
        for li, (nv, closed) in enumerate(zip(m["never"], world["never"])):
            if closed is None:
                STATED.append(f"machine: a `never` line{label} is not laid on the rows")
                continue
            a, b = set(nv["states"]), set(nv["after"])
            cl = {(s, bool(x)) for s, x in closed}
            if (init, init in b) not in cl:
                raise Bad(f"machine: a `never` set{label} leaves out where a case starts")
            for s, seen in cl:
                if seen and s in a:
                    raise Bad(f"machine: a `never` set{label} holds {states[s]} after the states it is to come after")
                for row, to in step(s):
                    if (to, seen or to in b) not in cl:
                        raise Bad(f"machine: a `never` set{label} is not closed at {states[s]}, row {row}")
            claims.add(f"never {li}")
        for li, (on_, closed) in enumerate(zip(m["once"], world["once"])):
            if closed is None or "test" not in on_:
                STATED.append(f"machine: `once {on_['output']}`{label} is not laid on the rows")
                continue
            test = on_["test"]

            def counts(v):
                if v is None:
                    return True
                if "words" in test:
                    w = ("true" if v else "false") if isinstance(v, bool) else str(v)
                    return w in test["words"]
                lo, hi = test.get("lo"), test.get("hi")
                return (lo is None or v >= lo) and (hi is None or v <= hi)

            flag = {r["row"]: counts(r["value"]) for r in on_["rows"]}
            if set(flag) != set(rows):
                raise Bad(f"machine: `once {on_['output']}` does not say what every row writes")
            cl = {(s, x) for s, x in closed}
            if (init, 0) not in cl:
                raise Bad(f"machine: the `once {on_['output']}` set{label} leaves out where a case starts")
            for s, cnt in cl:
                if cnt >= 2:
                    raise Bad(f"machine: the `once {on_['output']}` set{label} counts two")
                for row, to in step(s):
                    if (to, min(2, cnt + int(flag[row]))) not in cl:
                        raise Bad(f"machine: the `once {on_['output']}` set{label} is not closed at {states[s]}, row {row}")
            claims.add(f"once {li}")
        # A final state can still be reached, from every state in the reach set.
        if not finals:
            continue
        if coupled:
            if world["finish"]:
                raise Bad(f"machine: paths to a final state are given{label}, and a held input is read by more than its own column")
            STATED.append(f"machine: that a case{label} can still finish is not laid on the rows (a held input is read by more than its own column)")
            continue
        # A machine that carries more than its state (§15.217): a path of the state's rows is
        # not one case's, since each call moves the carried values too.
        if m.get("carried") is not None:
            if world["finish"]:
                raise Bad(f"machine: paths to a final state are given{label}, and the machine carries more than its state")
            STATED.append(f"machine: that a case{label} can still finish is not laid on the rows (the machine carries more than its state)")
            continue
        paths = {p["state"]: p["path"] for p in world["finish"]}
        for s in sorted(reach - finals):
            if s not in paths:
                STATED.append(f"machine: no path to a final state is given from {states[s]}{label}")
                continue
            at_state = s
            for st in paths[s]:
                row, at = st["row"], st["at"]
                if st["state"] != at_state or row not in rows:
                    raise Bad(f"machine: the path from {states[s]}{label} does not follow on at row {row}")
                if not takes(row):
                    raise Bad(f"machine: the path from {states[s]}{label} takes row {row}, which another world's calls make")
                if len(at) != len(axes) or any(c not in rows[row]["accepts"][ai] for ai, c in enumerate(at)):
                    raise Bad(f"machine: the call for row {row} is not in that row's box")
                if any(at[fk] != c for fk, c in zip(fixed, at_w)):
                    raise Bad(f"machine: the call for row {row} is not made with the world's held coordinates{label}")
                if k is not None and at[k] != at_state:
                    raise Bad(f"machine: the call for row {row} is not made from {states[at_state]}")
                why = sieve_admits(t, st.get("at_values"), at)
                if why is None:
                    why = facts_hold(t, t["_cert"], st.get("at_values"), st.get("extra_values"))
                if why is not None:
                    if st.get("at_values") is None or "no value is given" in why or "states no values" in why:
                        weak += 1
                    else:
                        raise Bad(f"machine: the call for row {row} is one no input makes — {why}")
                if t["policy"] == "first":
                    for earlier in sorted(x for x in rows if x < row):
                        if all(c in rows[earlier]["accepts"][ai] for ai, c in enumerate(at)):
                            raise Bad(f"machine: the call for row {row} is taken by row {earlier} first")
                at_state = at_state if moves[row] is None else moves[row]
            if at_state not in finals:
                raise Bad(f"machine: the path from {states[s]}{label} ends at {states[at_state]}, which is not final")
        claims.add("finish")
    if weak:
        STATED.append(f"machine: {weak} calls on the paths come with no values behind them")
    worlds = "" if not fixed else f" in {len(m['worlds'])} worlds"
    return (f"machine over {m['table']}: {len(reached)} of {n} states reached from {states[init]}{worlds}, "
            f"{len(claims)} claims laid on the rows")


class Inexact(Exception):
    """A rounding that meets a value off its grid: the value depends on the mode."""


def exact_value(e, env):
    """The value of an expression where every rounding meets a value already on its grid (§15.233),
    which every mode leaves where it is. None where a name it reads has no value; Inexact where a
    rounding meets a value off its grid, or a grid that is not positive."""
    if "name" in e:
        return env.get(e["name"])
    if "num" in e:
        return num(e.get("value"))
    if "lit" in e:
        return num(e.get("value")) if e.get("type") == "date" else None
    op = e.get("op")
    if op is not None:
        l = exact_value(e["l"], env)
        if op == "/" and "num" in e["r"]:
            k = num(e["r"].get("value"))
            return None if l is None or k is None or k == 0 else l / k
        r = exact_value(e["r"], env)
        if l is None or r is None:
            return None
        if op == "+":
            return l + r
        if op == "-":
            return l - r
        if op == "*":
            return l * r
        if op == "/":
            return None if r == 0 else l / r
        return Fraction(1 if {"<=": l <= r, "<": l < r, ">=": l >= r, ">": l > r, "==": l == r}[op] else 0)
    if "call" in e:
        fn, args = e["call"], e.get("args", [])
        vals = [exact_value(a, env) for a in args]
        if fn in ("min", "max") and len(vals) == 2:
            if None in vals:
                return None
            return min(vals) if fn == "min" else max(vals)
        if fn in ROUNDINGS and len(args) == 2:
            x, g = vals
            if x is None:
                return None
            if g is None or g <= 0 or (x / g).denominator != 1:
                raise Inexact(f"{fn}({x}, {g})")
            return x
        if fn == "mod" and len(vals) == 2:
            return None if None in vals else Fraction(math.floor(vals[0]) % int(vals[1]))
        if fn == "isqrt" and len(vals) == 1:
            return None if vals[0] is None else Fraction(math.isqrt(max(math.floor(vals[0]), 0)))
        if fn == "allocate" and len(vals) == 3:
            return None if None in vals or vals[2] == 0 else Fraction(math.floor(vals[0] * vals[1] / vals[2]))
    return None


def check_carried(cert):
    """Re-check the values a machine carries besides its state (§15.217). Returns a one-line
    summary, or None when the machine carries only its state; raises Bad.

    A place a case can be is the state and one coordinate of each carried value. Those
    coordinates are checked to cover each number's declared range on its grid without a gap or
    an overlap, and to be the two truth values or an enum's values. A row takes a place when
    its box takes the state and meets each carried value's coordinate on the column the table
    reads it in; it moves the state as the machine's own rows say, and each carried value to
    what the row writes into its output — read off the table here: the value, the value
    itself, or the interval a named computation comes to over the place, by the arithmetic
    the rest of this program does, narrowed by the row's box where the table reads it — rounded
    out to the grid its output rounds to (`round`, rulec's §15.232), since the rounded value is
    what the next call is passed, on every coordinate that meets it, and outside the range where
    it reaches past. The set of
    places must hold where a case starts and be closed under that; each claim is a closure of
    the same kind, as the machine's are. An `always` line speaks of the places within the
    ranges; a place outside one is the range's claim. That a case can still finish needs calls
    that really happen: where the section gives a path of them from every place, each call is run
    here (§15.233).
    """
    m = cert["machine"]
    cm = m.get("carried")
    if cm is None:
        return None
    if "items" not in cm:
        STATED.append(f"machine: the carried values are not laid on the rows — {cm.get('why', '')}")
        return "carried values: not certified here — " + cm.get("why", "")
    t = next((x for x in cert["tables"] if x["table"] == m["table"]), None)
    if t is None:
        raise Bad(f"carried values: the table {m['table']} has no certificate")
    rows = {r["row"]: r for r in t["rows"]}
    order = [r["row"] for r in t["rows"]]
    axes, decides, states, k = t["axes"], t["decides"], m["states"], m["axis"]
    moves = {r["row"]: r.get("to") for r in m["rows"]}
    on = {row: (rows[row]["accepts"][k] if k is not None else list(range(len(states)))) for row in rows}
    values = {v["name"]: v["expr"] for v in cert.get("values", [])}
    vorder = [v["name"] for v in cert.get("values", [])]
    base = {}
    for n in cert.get("inputs", []):
        r = cert.get("ranges", {}).get(n)
        if r is not None and r[0] is not None and r[1] is not None:
            base[n] = (r[0], r[1])
    items = cm["items"]
    coords, labels, mv, grids = [], [], [], []
    for it in items:
        name = it["input"]
        cs = [(Fraction(a), Fraction(b)) for a, b in it["coords"]]
        if it["output"] not in decides:
            raise Bad(f"carried values: {m['table']} does not decide {it['output']}")
        j = decides.index(it["output"])
        ax = it.get("axis")
        named = [i for i, a in enumerate(axes) if a["column"] == name]
        if (ax is None and named) or (ax is not None and ax not in named):
            raise Bad(f"carried values: the column {m['table']} reads {name} in is not the one named")
        kind = it["kind"]
        if kind == "bool":
            if cs != [(0, 0), (1, 1)] or it.get("labels") != ["false", "true"]:
                raise Bad(f"carried values: {name} is a truth value, and its coordinates are not false and true")
        elif kind == "enum":
            ls = it.get("labels") or []
            if cs != [(i, i) for i in range(len(ls))] or (ax is not None and axes[ax]["coords"] != ls):
                raise Bad(f"carried values: the coordinates of {name} are not its enum's values in order")
        else:
            lo, hi = Fraction(it["range"][0]), Fraction(it["range"][1])
            declared = cert.get("ranges", {}).get(name)
            if declared is None or num(declared[0]) != lo or num(declared[1]) != hi:
                raise Bad(f"carried values: the range of {name} is not the one it declares")
            step = Fraction(it["step"])
            if not cs or cs[0][0] != lo or cs[-1][1] != hi or any(a > b for a, b in cs):
                raise Bad(f"carried values: the coordinates of {name} do not run from one end of its range to the other")
            for (_, b), (c, _) in zip(cs, cs[1:]):
                if c != b + step:
                    raise Bad(f"carried values: the coordinates of {name} leave a gap or overlap at {b}")
        # The grid the output rounds to (§7.2): the call passes on the rounding of what the row
        # writes, so that is what lands on a coordinate (rulec's §15.232). A certificate from before
        # it was written lands the value as it is.
        grid = None
        if kind not in ("bool", "enum"):
            if it.get("round") is None:
                STATED.append(f"carried values: the certificate does not say what grid {it['output']} rounds to, "
                              f"so its values land as the rows write them")
            else:
                grid = Fraction(it["round"])
                if grid <= 0:
                    raise Bad(f"carried values: {it['output']} rounds to a grid that does not move")
        got = {}
        for r in it["moves"]:
            row = r["row"]
            if row not in rows:
                raise Bad(f"carried values: row {row} is not in {m['table']}")
            if kind == "number":
                w = (rows[row].get("writes") or [None] * len(decides))[j]
                e = None if w is None else w.get("expr")
                if r.get("stay"):
                    ok = e == {"name": name}
                    got[row] = ("stay",)
                elif "expr" in r:
                    ok = e == {"name": r["expr"]} and r["expr"] in values
                    got[row] = ("expr", r["expr"])
                else:
                    v = None if e is None or "num" not in e else num(e.get("value"))
                    inside = [] if v is None else [ci for ci, (a, b) in enumerate(cs) if a <= v <= b]
                    if r.get("out"):
                        ok = v is not None and not inside
                    else:
                        ok = r.get("to") in inside
                    got[row] = ("lit", v)
            else:
                produced = rows[row]["produces"][j]
                if r.get("stay"):
                    ok = produced is None
                    got[row] = ("stay",)
                else:
                    ok = r.get("to") is not None and produced == it["labels"][r["to"]]
                    got[row] = ("to", r.get("to"))
            if not ok:
                raise Bad(f"carried values: what row {row} is said to write into {it['output']} is not what the table says")
        if set(got) != set(rows):
            raise Bad(f"carried values: rows {sorted(set(rows) - set(got))} say nothing of {it['output']}")
        coords.append(cs)
        labels.append(it.get("labels"))
        mv.append(got)
        grids.append(grid)
    OUT = -1

    def land(kk, lo, hi):
        # Where a carried number lands: the interval rounded out to the output's grid, every
        # coordinate that meets it, and outside the range where it reaches past.
        g = grids[kk]
        if g is not None:
            lo, hi = (lo // g) * g, -((-hi) // g) * g
        ls = [ci for ci, (a, b) in enumerate(coords[kk]) if a <= hi and b >= lo]
        if lo < coords[kk][0][0] or hi > coords[kk][-1][1]:
            ls.append(OUT)
        return ls

    def meets(row, kk, ci):
        ax = items[kk].get("axis")
        if ax is None:
            return True
        acc = rows[row]["accepts"][ax]
        if labels[kk] is not None:
            return any(axes[ax]["coords"][x] == labels[kk][ci] for x in acc)
        lo, hi = coords[kk][ci]
        for x in acc:
            blo, bhi = closed_at(t, ax, x)
            if (blo is None or blo <= hi) and (bhi is None or bhi >= lo):
                return True
        return False

    def image(row, cc, name):
        ranges = dict(base)
        for kk, it in enumerate(items):
            if labels[kk] is None:
                a, b = coords[kk][cc[kk]]
                ranges[it["input"]] = (str(a), str(b))
        for _ in range(len(vorder) + 1):
            moved = False
            for n in vorder:
                if n in ranges:
                    continue
                iv = interval(values[n], ranges)
                if iv is not None:
                    ranges[n] = (str(iv[0]), str(iv[1]))
                    moved = True
            if not moved:
                break
        if name not in ranges:
            return None
        lo, hi = num(ranges[name][0]), num(ranges[name][1])
        cols = [a["column"] for a in axes]
        if name in cols:
            ai = cols.index(name)
            bs = [closed_at(t, ai, x) for x in rows[row]["accepts"][ai]]
            if bs and all(b[0] is not None for b in bs):
                lo = max(lo, min(b[0] for b in bs))
            if bs and all(b[1] is not None for b in bs):
                hi = min(hi, max(b[1] for b in bs))
        return lo, hi

    first = t["policy"] == "first"

    def covers(r0, row, s, cc):
        # Under `policy first`, an earlier row that alone takes the place's state, the whole of
        # each carried value's coordinate, and on every other column all the later row takes.
        if s not in on[r0]:
            return False
        for ai, ax in enumerate(axes):
            if ai == k:
                continue
            a0, a1 = rows[r0]["accepts"][ai], rows[row]["accepts"][ai]
            kk = next((x for x in range(len(cc)) if items[x].get("axis") == ai), None)
            if kk is None:
                if not set(a1) <= set(a0):
                    return False
                continue
            if labels[kk] is not None:
                whole = [x for x in range(len(ax["coords"])) if ax["coords"][x] == labels[kk][cc[kk]]]
            else:
                lo, hi = coords[kk][cc[kk]]
                whole = []
                for x in range(len(ax["coords"])):
                    blo, bhi = closed_at(t, ai, x)
                    if (blo is None or blo <= hi) and (bhi is None or bhi >= lo):
                        whole.append(x)
            if not set(whole) <= set(a0):
                return False
        return True

    def step(p):
        s, cc = p
        if OUT in cc:
            return []
        nxt = []
        for i, row in enumerate(order):
            if s not in on[row] or not all(meets(row, kk, cc[kk]) for kk in range(len(cc))):
                continue
            if first and any(covers(r0, row, s, cc) for r0 in order[:i]):
                continue
            s2 = s if moves.get(row) is None else moves[row]
            lists = []
            for kk in range(len(cc)):
                g = mv[kk][row]
                if g[0] == "stay" and labels[kk] is not None:
                    lists.append([cc[kk]])
                elif g[0] == "to":
                    lists.append([g[1]])
                elif g[0] == "stay":
                    a, b = coords[kk][cc[kk]]
                    lists.append(land(kk, a, b))
                elif g[0] == "lit":
                    if g[1] is None:
                        raise Bad(f"carried values: what row {row} writes into {items[kk]['output']} cannot be read")
                    lists.append(land(kk, g[1], g[1]))
                else:
                    iv = image(row, cc, g[1])
                    if iv is None:
                        raise Bad(f"carried values: what {g[1]} comes to at {states[s]} is not bounded here")
                    lists.append(land(kk, iv[0], iv[1]))
            for combo in itertools.product(*lists):
                nxt.append((row, (s2, tuple(combo))))
        return nxt

    def place(x):
        return (x[0], tuple(x[1]))

    places = {place(x) for x in cm["places"]}
    init, finals = m["initial"], set(m["finals"])
    start = (init, tuple(cm["start"]))
    if start not in places:
        raise Bad("carried values: the places leave out where a case starts")
    succ = {}
    for p in places:
        succ[p] = step(p)
        for row, q in succ[p]:
            if q not in places:
                raise Bad(f"carried values: from {states[p[0]]}, row {row} leads outside the places")
    claims = ["reach"]
    if cm.get("final_certified"):
        for p in places:
            if p[0] in finals and any(q[0] != p[0] for _, q in succ[p]):
                raise Bad(f"carried values: a row leads out of the final state {states[p[0]]}")
        claims.append("final")
    elif any(p[0] in finals for p in places):
        STATED.append("carried values: that no call leaves a final state is not laid on the rows")
    for li, (nv, closed) in enumerate(zip(m["never"], cm.get("never", []))):
        if closed is None:
            STATED.append(f"carried values: the `never` line {li + 1} is not laid on the rows")
            continue
        a, b = set(nv["states"]), set(nv["after"])
        cl = {(x[0], tuple(x[1]), bool(x[2])) for x in closed}
        if (start[0], start[1], start[0] in b) not in cl:
            raise Bad("carried values: a `never` set leaves out where a case starts")
        for s, cc, seen in cl:
            if seen and s in a:
                raise Bad(f"carried values: a `never` set holds {states[s]} after the states it is to come after")
            for row, q in succ.get((s, cc), []):
                if (q[0], q[1], seen or q[0] in b) not in cl:
                    raise Bad(f"carried values: a `never` set is not closed at {states[s]}, row {row}")
        claims.append(f"never {li}")
    for li, (on_, closed) in enumerate(zip(m["once"], cm.get("once", []))):
        if closed is None or "test" not in on_:
            STATED.append(f"carried values: `once {on_['output']}` is not laid on the rows")
            continue
        test = on_["test"]

        def counts(v, test=test):
            if v is None:
                return True
            if "words" in test:
                w = ("true" if v else "false") if isinstance(v, bool) else str(v)
                return w in test["words"]
            lo, hi = test.get("lo"), test.get("hi")
            return (lo is None or v >= lo) and (hi is None or v <= hi)

        flag = {r["row"]: counts(r["value"]) for r in on_["rows"]}
        cl = {(x[0], tuple(x[1]), x[2]) for x in closed}
        if (start[0], start[1], 0) not in cl:
            raise Bad(f"carried values: the `once {on_['output']}` set leaves out where a case starts")
        for s, cc, cnt in cl:
            if cnt >= 2:
                raise Bad(f"carried values: the `once {on_['output']}` set counts two")
            for row, q in succ.get((s, cc), []):
                if (q[0], q[1], min(2, cnt + int(flag.get(row, True)))) not in cl:
                    raise Bad(f"carried values: the `once {on_['output']}` set is not closed at {states[s]}, row {row}")
        claims.append(f"once {li}")

    def fits(cell, kk, ci):
        if ci == OUT:
            return False
        lo, hi = coords[kk][ci]
        kind = cell.get("cell")
        if labels[kk] is not None:
            w = labels[kk][ci]
            if kind == "is":
                return w in cell["words"]
            if kind == "not":
                return w not in cell["words"]
            return kind == "any"
        if kind == "cmp":
            ops = {"<=": lambda x, v: x <= v, "<": lambda x, v: x < v, ">=": lambda x, v: x >= v, ">": lambda x, v: x > v}
            return all(t2["value"] is not None and ops[t2["op"]](lo, num(t2["value"])) and ops[t2["op"]](hi, num(t2["value"]))
                       for t2 in cell["tests"])
        if kind == "in":
            return lo == hi and any(v is not None and num(v) == lo for v in cell["values"])
        if kind == "not_in":
            return all(v is not None and (num(v) < lo or num(v) > hi) for v in cell["values"])
        return kind == "any"

    for li, al in enumerate(cm.get("always", [])):
        when = set(al["when"])
        # A place outside a range is the range's claim, not this one's.
        held = all(all(fits(tt["cell"], tt["item"], p[1][tt["item"]]) for tt in al["tests"])
                   for p in places if (not when or p[0] in when) and OUT not in p[1])
        if al.get("holds"):
            if not held:
                raise Bad(f"carried values: the `always` line {li + 1} does not hold at every place it names")
            claims.append(f"always {li}")
        else:
            STATED.append(f"carried values: the `always` line {li + 1} is not laid on the rows")
    for rg in cm.get("ranges", []):
        kk = rg["item"]
        if rg.get("holds"):
            if any(p[1][kk] == OUT for p in places):
                raise Bad(f"carried values: {items[kk]['output']} is said to stay within its range, and a place leaves it")
            claims.append(f"range {kk}")
        else:
            STATED.append(f"carried values: that {items[kk]['output']} stays within its range is not laid on the rows")
    # That a case can still finish (§15.233): from every place not in a final state — one value per
    # coordinate, where every carried number was walked one value at a time — a path of calls that
    # really happen. Each call is run here: the row's box takes the point at the place's state, and
    # under `first` no earlier row does; the point's values pass the sieve; every column is an input,
    # or a number the rule computes, inside its coordinate; the inputs keep their ranges and the
    # constraints; the values the rule computes are worked out from the inputs with every rounding
    # meeting a value on its grid; and each carried value moves on to exactly what the row writes.
    if finals and cm.get("finish") is not None:
        cols = [a["column"] for a in axes]
        inputs = set(cert.get("inputs", []))
        extras = (t.get("linear") or {}).get("extra", [])
        paths = {place(x["place"]): x["path"] for x in cm["finish"]}

        def run_call(s, vals, st):
            row, at = st["row"], st["at"]
            if row not in rows:
                raise Bad(f"carried values: a call on a path names row {row}, which is not in {m['table']}")
            if len(at) != len(axes) or any(not 0 <= c < len(axes[ai]["coords"]) for ai, c in enumerate(at)):
                raise Bad(f"carried values: the call for row {row} is not a point of the table")
            if any(c not in rows[row]["accepts"][ai] for ai, c in enumerate(at)):
                raise Bad(f"carried values: the call for row {row} is not in that row's box")
            if k is not None and at[k] != s:
                raise Bad(f"carried values: the call for row {row} is not made from {states[s]}")
            if first and any(all(c in rows[e]["accepts"][ai] for ai, c in enumerate(at)) for e in order if e < row):
                raise Bad(f"carried values: the call for row {row} is taken by an earlier row first")
            env = {}
            for n, v in (st.get("inputs") or {}).items():
                if n not in inputs:
                    raise Bad(f"carried values: the call for row {row} is made with {n}, which is not an input")
                env[n] = Fraction(v)
            try:
                for _ in range(len(vorder) + 1):
                    for n in vorder:
                        if n not in env:
                            v = exact_value(values[n], env)
                            if v is not None:
                                env[n] = v
                for n in vorder:
                    if exact_value(values[n], env) != env.get(n):
                        raise Bad(f"carried values: the call for row {row}: {n} is not what its expression comes to")
            except Inexact as why:
                raise Bad(f"carried values: the call for row {row} rounds a value off its grid ({why})")
            for ai, (a, c) in enumerate(zip(axes, at)):
                b = closed_at(t, ai, c) if (a.get("bounds") or [None] * len(a["coords"]))[c] is not None else None
                if a["column"] not in inputs and (a["column"] not in values or b is None):
                    raise Bad(f"carried values: the call for row {row} is laid on a table that reads {a['column']}, "
                              f"which is neither an input nor a number the rule computes")
                if b is not None:
                    v = env.get(a["column"])
                    if v is None or (b[0] is not None and v < b[0]) or (b[1] is not None and v > b[1]):
                        raise Bad(f"carried values: the call for row {row}: {a['column']} is not inside its coordinate")
            for n in inputs:
                r = cert.get("ranges", {}).get(n)
                if n in env and r is not None and r[0] is not None and r[1] is not None and not num(r[0]) <= env[n] <= num(r[1]):
                    raise Bad(f"carried values: the call for row {row} is made with {n} outside its range")
            for kc in cert.get("constraints", []):
                ls, rs = side(kc["left"]), side(kc["right"])
                if any(n not in env for n in ls + rs):
                    raise Bad(f"carried values: the call for row {row} does not say {side_text(kc['left'])} {kc['op']} {side_text(kc['right'])}")
                x, y = sum((env[n] for n in ls), Fraction(0)), sum((env[n] for n in rs), Fraction(0))
                if not {"<=": x <= y, "<": x < y, ">=": x >= y, ">": x > y}[kc["op"]]:
                    raise Bad(f"carried values: the call for row {row} breaks {side_text(kc['left'])} {kc['op']} {side_text(kc['right'])}")
            at_values = [None if (a.get("bounds") or [None] * len(a["coords"]))[c] is None else str(env.get(a["column"]))
                         for a, c in zip(axes, at)]
            why = sieve_admits(t, at_values, at) or facts_hold(t, cert, at_values, [str(env[n]) if n in env else None for n in extras])
            if why is not None:
                raise Bad(f"carried values: the call for row {row} is one no input makes — {why}")
            nxt = []
            for kk, it in enumerate(items):
                v = vals[kk]
                ax = it.get("axis")
                if labels[kk] is None:
                    if env.get(it["input"]) != v:
                        raise Bad(f"carried values: the call for row {row} is not made with {it['input']} = {v}")
                    if ax is not None:
                        lo, hi = closed_at(t, ax, at[ax])
                        if (lo is not None and v < lo) or (hi is not None and v > hi):
                            raise Bad(f"carried values: the call for row {row}: {it['input']} is not inside its coordinate")
                elif ax is not None and axes[ax]["coords"][at[ax]] != labels[kk][int(v)]:
                    raise Bad(f"carried values: the call for row {row} is not made with {it['input']} = {labels[kk][int(v)]}")
                g = mv[kk][row]
                if g[0] == "stay":
                    w = v
                elif g[0] == "to":
                    w = Fraction(g[1])
                elif g[0] == "lit":
                    w = g[1]
                else:
                    w = env.get(g[1])
                if w is None or (labels[kk] is None and grids[kk] is not None and (w / grids[kk]).denominator != 1):
                    raise Bad(f"carried values: the call for row {row} carries {it['output']} on to a value off its grid")
                nxt.append(w)
            return (s if moves.get(row) is None else moves[row]), nxt

        for p in sorted(places):
            if p[0] in finals:
                continue
            if any(c == OUT or coords[kk][c][0] != coords[kk][c][1] for kk, c in enumerate(p[1])):
                raise Bad(f"carried values: a place at {states[p[0]]} stands for more than one value, and a path is given from it")
            if p not in paths:
                raise Bad(f"carried values: no path to a final state is given from a place at {states[p[0]]}")
            s, vals = p[0], [coords[kk][c][0] for kk, c in enumerate(p[1])]
            for st in paths[p]:
                s, vals = run_call(s, vals, st)
            if s not in finals:
                raise Bad(f"carried values: the path from a place at {states[p[0]]} ends at {states[s]}, which is not final")
        claims.append("finish")
    # A claim laid here is laid: the closure over the state alone could not carry it (its rows
    # move the carried values too), and it is no longer listed as stated for that.
    laid_once = {m["once"][int(c.split()[1])]["output"] for c in claims if c.startswith("once ")}
    all_never = bool(m["never"]) and all(f"never {i}" in claims for i in range(len(m["never"])))
    STATED[:] = [x for x in STATED if not (
        ("final" in claims and x.startswith("machine: that no call leaves a final state"))
        or (all_never and x.startswith("machine: a `never` line"))
        or ("finish" in claims and x.startswith("machine: that a case") and "carries more than its state" in x)
        or any(x.startswith(f"machine: `once {o}`") for o in laid_once))]
    return f"carried values over {m['table']}: {len(places)} places a case can be, {len(claims)} claims laid on the rows"


def literal_number(text, ty):
    """The number a literal written in the rule stands for, in a column of type `ty`: `%` a
    hundredth, no unit or the column's currency the number itself. None otherwise."""
    k = lit_key(text)
    if k[0] != "n":
        return None
    n, unit = k[1], k[2]
    cur = ty[len("money["):-1].split(",")[0].strip() if isinstance(ty, str) and ty.startswith("money[") else None
    if unit == "%":
        return n / 100
    if unit == "" or (cur is not None and unit == cur):
        return n
    return None


def check_machine_cells(cert, path):
    """The machine's section says what the file says (§15.148, §15.233): a row said to leave the
    state where it is writes the carried input's own name, a row said to go to a state writes
    that state, each in the row's answer cell of the carried output; a value carried besides the
    state the same way; the value a `once` line reads on each row is the one its answer cell
    writes; and where a case starts is what the `initial` lines say."""
    with open(path, "rb") as fh:
        lines = fh.read().split(b"\n")
    m = cert["machine"]
    t = next((x for x in cert["tables"] if x["table"] == m["table"]), {})
    decides = t.get("decides") or []
    types = cert.get("types") or {}

    def text(src):
        ln = lines[src["line"] - 1] if 0 < src["line"] <= len(lines) else b""
        return ln[src["col"]:src["col"] + src["len"]].decode("utf-8", "replace")

    def answer_cell(row, column, src):
        """Whether `src` is the row's answer cell of `column`: on the row's own line, the cell the
        column's place puts after the cells its tests read. A row whose answers are not on its own
        line (a `clause`) has none, and only what the file says there is held."""
        r = next((x for x in t.get("rows", []) if x["row"] == row), None)
        if r is None or column not in decides or r.get("source") is None:
            return False
        ln = r.get("line", 0)
        line = lines[ln - 1] if 0 < ln <= len(lines) else b""
        bars = fields_of(line)
        if not bars:
            return True
        k = len([sp for sp in r["source"] if sp is not None]) + decides.index(column)
        return src.get("line") == ln and k < len(bars) and bars[k] == (src["col"], src["col"] + src["len"])

    def initial_line(src, before):
        """Whether the place is the value of an `initial` line: the line reads `initial`, then
        `before`, then the value, and nothing after it but a comment."""
        ln = lines[src["line"] - 1] if 0 < src["line"] <= len(lines) else b""
        head = ln[:src["col"]].decode("utf-8", "replace").split()
        tail = ln[src["col"] + src["len"]:].decode("utf-8", "replace").strip()
        return head == ["initial"] + before and (not tail or tail.startswith("#"))

    read = 0
    out = m["carry"]["output"]
    for r in m["rows"]:
        src = r.get("source")
        if src is None:
            raise Bad(f"machine: row {r['row']}'s move has no place in the file")
        got = text(src)
        want = m["carry"]["input"] if r.get("stay") else m["states"][r["to"]]
        if got != want or got != src.get("text"):
            raise Bad(f"machine: row {r['row']} is said to write `{want}`, and the file has `{got}` there")
        if not answer_cell(r["row"], out, src):
            raise Bad(f"machine: row {r['row']}'s move is not read from the row's own cell of {out}")
        read += 1
    for on in m["once"]:
        test = on.get("test")
        if test is None:
            continue
        for r in on.get("rows", []):
            src = r.get("source")
            if src is None:
                raise Bad(f"machine: the value `once {on['output']}` reads on row {r['row']} has no place in the file")
            if text(src) != src.get("text"):
                raise Bad(f"machine: the value `once {on['output']}` reads on row {r['row']} is not what the file has there")
            if not answer_cell(r["row"], on["output"], src):
                raise Bad(f"machine: the value `once {on['output']}` reads on row {r['row']} is not read from the row's own cell of {on['output']}")
            v, said = r.get("value"), text(src).strip()
            # A word is one of the output's own values, `none` among them where the output is
            # optional (rulec's §15.249). A cell that names something the rule computes holds no
            # word of its own: read as one, the row would count nowhere, wherever it may.
            if isinstance(v, str) and "words" in test:
                ty = types.get(on["output"], "")
                base = ty[:-1] if ty.endswith("?") else ty
                values = (["none"] if ty.endswith("?") else []) + list((cert.get("enums") or {}).get(base, []))
                if v not in values:
                    raise Bad(f"machine: the value `once {on['output']}` reads on row {r['row']}, `{v}`, is not a value of {on['output']}")
            # A computed value counts wherever it might: nothing to hold it to.
            if v is not None:
                if "words" not in test:
                    if test.get("scale") is None:
                        STATED.append(f"the values the `once {on['output']}` line reads: the certificate does not say the scale they are counted at")
                    else:
                        x = literal_number(said, types.get(on["output"]))
                        if x is None or x * test["scale"] != v:
                            raise Bad(f"machine: row {r['row']} writes `{said}` into {on['output']}, and the certificate reads another value for the `once` line")
                elif isinstance(v, bool):
                    if said != ("true" if v else "false"):
                        raise Bad(f"machine: row {r['row']} writes `{said}` into {on['output']}, and the certificate reads another value for the `once` line")
                elif said != v:
                    raise Bad(f"machine: row {r['row']} writes `{said}` into {on['output']}, and the certificate reads another value for the `once` line")
            read += 1
    # The values carried besides the state (§15.217): a row said to keep one writes its own name,
    # a row said to write a truth value or an enum's writes it, a computation is named.
    cm = m.get("carried") or {}
    for it in cm.get("items", []):
        for r in it["moves"]:
            src = r.get("source")
            if src is None:
                raise Bad(f"carried values: row {r['row']}'s move of {it['output']} has no place in the file")
            got = text(src)
            if r.get("stay"):
                want = it["input"]
            elif "expr" in r:
                want = r["expr"]
            elif it.get("labels") is not None and "to" in r:
                want = it["labels"][r["to"]]
            else:
                want = src.get("text")
            if got != want or got != src.get("text"):
                raise Bad(f"carried values: row {r['row']} is said to write `{want}` into {it['output']}, and the file has `{got}` there")
            if not answer_cell(r["row"], it["output"], src):
                raise Bad(f"carried values: row {r['row']}'s move of {it['output']} is not read from the row's own cell of it")
            read += 1
    # Where a case starts (§15.233): the state the `initial` line names, and for each value carried
    # besides it, the value its own `initial` line gives, inside the coordinate the start gives it.
    src = m.get("initial_source")
    if src is None:
        STATED.append("where a case starts: the certificate does not say where the `initial` line is")
    else:
        want = m["states"][m["initial"]] if 0 <= m["initial"] < len(m["states"]) else None
        if text(src) != want or text(src) != src.get("text"):
            raise Bad(f"machine: the `initial` line does not name {want} there")
        if not initial_line(src, []):
            raise Bad("machine: where a case starts is not read from an `initial` line")
        read += 1
    for k, it in enumerate(cm.get("items", [])):
        src = it.get("initial_source")
        if src is None:
            STATED.append(f"where {it['input']} starts: the certificate does not say where its `initial` line is")
            continue
        got = text(src)
        if got != src.get("text"):
            raise Bad(f"carried values: the `initial` line of {it['input']} is not what the file has there")
        if not initial_line(src, [it["input"]]):
            raise Bad(f"carried values: where {it['input']} starts is not read from its `initial` line")
        start = (cm.get("start") or [])[k] if k < len(cm.get("start") or []) else None
        said = got.strip()
        if it.get("labels") is not None:
            ok = start is not None and 0 <= start < len(it["labels"]) and it["labels"][start] == said
        else:
            x = literal_number(said, types.get(it["input"]))
            co = it["coords"][start] if start is not None and 0 <= start < len(it["coords"]) else None
            ok = x is not None and co is not None and num(co[0]) <= x <= num(co[1])
        if not ok:
            raise Bad(f"carried values: {it['input']} starts at `{said}` by its `initial` line, and the certificate starts it elsewhere")
        read += 1
    return f"the machine's moves say the same as the file: {read} cells read back"


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
    # Every value the rule computes and every column a table writes numbers into, worked out
    # here: a value from its expression, a column from what its rows write (§15.196).
    known, outs, unstated = worked_ranges(cert)
    try:
        held = check_outputs(cert, known, outs)
        if held:
            out.append(f"  ranges: {held} columns a table writes numbers into hold every value their rows write")
        if unstated:
            STATED.append(f"the ranges of {', '.join(unstated)}: their rows state nothing they write")
    except Bad as e:
        out.append(f"  FAILED ranges: {e}")
        ok = False
    try:
        checked, none_stated = check_values(cert, known)
        rest = f", {none_stated} with no interval to claim" if none_stated else ""
        out.append(f"  int64: {checked} values fit{rest}")
    except Bad as e:
        out.append(f"  FAILED int64: {e}")
        ok = False
    if cert.get("extremes"):
        try:
            held = check_extremes(cert, known)
            out.append(f"  extremes: {held} `max` and `min` answers keep to their column's range and their `empty` value")
        except Bad as e:
            out.append(f"  FAILED extremes: {e}")
            ok = False
    if cert.get("fold_answers"):
        try:
            held = check_fold_answers(cert, known)
            out.append(f"  fold answers: {held} ranges hold every value their fold answers with")
        except Bad as e:
            out.append(f"  FAILED fold answers: {e}")
            ok = False
    if cert.get("opens"):
        try:
            held, terms, by = check_opens(cert, known)
            out.append(f"  open values: {held} hold what their {terms} terms come to together inside the range they declare"
                       + (f", {by} of the terms from contributions" if by else ""))
        except Bad as e:
            out.append(f"  FAILED open values: {e}")
            ok = False
    # The reach of a derived value is the interval its own expression is forced into — the
    # cover's "out of reach" leaves are held to that, recomputed here.
    computed = {v["name"] for v in cert.get("values", [])}
    reach = {k: iv for k, iv in known.items() if k in computed}
    written = {k: iv for k, iv in known.items() if k in outs}
    # The truth value a boolean `define` of one comparison always takes, where it takes one
    # (§15.196), over the intervals worked out here.
    env = {k: (None if a is None else a, None if b is None else b) for k, (a, b) in ranges.items() if k not in outs}
    env.update({k: (str(a), str(b)) for k, (a, b) in known.items()})
    truth = {}
    for v in cert.get("values", []):
        if v.get("type") == "bool" and v.get("of", "define") == "define":
            tv = truth_of(v.get("expr"), env, guarantees(cert))
            if tv is not None:
                truth[v["name"]] = tv
    # The kind of every column, worked out here from `inputs`, `walks` and `values` (§15.196);
    # a certificate from before it carried them leaves the kinds its own word.
    kinds = column_kinds(cert) if "inputs" in cert else None
    if kinds is None:
        STATED.append("the kinds of the columns: the certificate does not say what the rule's inputs are")
    if not cert["tables"]:
        out.append("  no table states a certificate")
    for t in cert["tables"]:
        if kinds is not None:
            try:
                check_kinds(t, kinds)
            except Bad as e:
                out.append(f"  FAILED {e}")
                ok = False
        t["_reach_of"] = reach
        t["_written"] = written
        t["_truth"] = truth
        t["_tables"] = cert["tables"]
        t["_ranges"] = cert.get("ranges", {})
        # The days a date input takes from a koyomi file (§15.174) are the document's word, as
        # the ranges are: tied to the koyomi file by its SHA-256, which this program does not run.
        t["_days"] = cert.get("days", {})
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
    for k in cert.get("contracts", []):
        try:
            out.append("  " + check_contract(cert, k))
        except Bad as e:
            out.append(f"  FAILED {e}")
            ok = False
    if cert.get("machine"):
        try:
            out.append("  " + check_machine(cert))
        except Bad as e:
            out.append(f"  FAILED {e}")
            ok = False
        try:
            line = check_carried(cert)
            if line:
                out.append("  " + line)
        except Bad as e:
            out.append(f"  FAILED {e}")
            ok = False
    for p in cert.get("prerequisites") or []:
        try:
            out.append("  " + check_prerequisites(cert, p))
        except Bad as e:
            out.append(f"  FAILED {e}")
            ok = False
    if cert.get("chances"):
        try:
            out.append("  " + check_chances(cert, cert["chances"]))
        except Bad as e:
            out.append(f"  FAILED {e}")
            ok = False
    return out, ok


def chance_value(v):
    """A value of a claim's output as the certificate writes it: a rational, a word, or None."""
    if v is None:
        return None
    try:
        return Fraction(v)
    except (ValueError, ZeroDivisionError):
        return v


def cell_passes(cell, v):
    """Whether one value passes one cell of a claim (rulec's §15.224): the shapes the
    certificate writes a cell in, numbers in the column's unit."""
    if "any" in cell:
        return True
    if "nothing" in cell:
        return v is None or v == "none"
    if v is None:
        return False
    if "words" in cell:
        return str(v) in cell["words"]
    if "not_words" in cell:
        return str(v) not in cell["not_words"]
    if "strs" in cell:
        return str(v) in cell["strs"]
    if "prefix" in cell:
        return any(str(v).startswith(p) for p in cell["prefix"])
    x = chance_value(v)
    if not isinstance(x, Fraction):
        return False
    if "nums" in cell:
        return any(x == Fraction(n) for n in cell["nums"])
    if "not_nums" in cell:
        return all(x != Fraction(n) for n in cell["not_nums"])
    if "cmp" in cell:
        return all({"<=": x <= Fraction(b), "<": x < Fraction(b), ">=": x >= Fraction(b), ">": x > Fraction(b)}[op]
                   for op, b in cell["cmp"])
    raise Bad(f"a cell this program cannot read: {cell}")


def coord_span(co, step):
    """The closed span of whole steps a coordinate of an axis holds, or None for a word."""
    if "point" in co:
        v = Fraction(co["point"])
        return (v, v)
    if "open" in co:
        lo, hi = co["open"]
        return (None if lo is None else Fraction(lo) + step, None if hi is None else Fraction(hi) - step)
    return None


def coord_in_cell(axis, co, cell):
    """Whether a whole coordinate of an input's axis lies inside a claim's cell. The axis is cut
    at every value the claims name, so a coordinate is wholly inside or wholly outside."""
    if "word" in co:
        return cell_passes(cell, co["word"])
    if "other" in co:
        return "any" in cell
    step = Fraction(axis["step"])
    lo, hi = coord_span(co, step)
    ends = [e for e in (lo, hi) if e is not None]
    if not ends:
        return "any" in cell
    inside = [cell_passes(cell, str(e)) for e in ends]
    if len(set(inside)) != 1:
        raise Bad(f"{axis['column']}: a coordinate is cut by a claim's cell, and the axes say it is not")
    return inside[0]


# What the answers read, as the Lean program's `RulecCert.Chances` reads it (§15.243). A coordinate is
# ("x", a), one number; ("b", lo, hi), the numbers strictly between two ends, either of which may be
# None; or ("w", word). A cell holds ("run", k, i, j), the whole steps of 1/k from the i-th to the j-th;
# ("coord", c), every value of a coordinate; or ("val", v), one value.

def chance_rin(cert, name):
    """What a column takes or a random input is drawn from: ("num", lo, hi, k) — the whole steps of
    1/k inside the range — or ("words", [...]); None for a string, an optional value, a number with no
    two ends."""
    ty = str((cert.get("types") or {}).get(name, ""))
    if ty.endswith("?"):
        return None
    if ty == "bool":
        return ("words", ["false", "true"])
    enums = cert.get("enums") or {}
    if ty in enums:
        er = (cert.get("enum_ranges") or {}).get(name)
        return ("words", list(er) if er is not None else list(enums[ty]))
    if ty == "string":
        return None
    r = (cert.get("ranges") or {}).get(name)
    k = (cert.get("scales") or {}).get(name)
    if r is None or r[0] is None or r[1] is None or not k:
        return None
    return ("num", Fraction(r[0]), Fraction(r[1]), int(k))


def rin_count(rin):
    if rin[0] == "num":
        _, lo, hi, k = rin
        return max(0, math.floor(hi * k) - math.ceil(lo * k) + 1)
    return len(rin[1])


def rin_grid(rin):
    if rin[0] == "num":
        _, lo, hi, k = rin
        return [Fraction(n, k) for n in range(math.ceil(lo * k), math.floor(hi * k) + 1)]
    return list(rin[1])


def wcoord(co):
    if "point" in co:
        return ("x", Fraction(co["point"]))
    if "open" in co:
        lo, hi = co["open"]
        return ("b", None if lo is None else Fraction(lo), None if hi is None else Fraction(hi))
    if "word" in co:
        return ("w", co["word"])
    return None


def wholds(c, v):
    if c[0] == "w":
        return isinstance(v, str) and v == c[1]
    if not isinstance(v, Fraction):
        return False
    if c[0] == "x":
        return v == c[1]
    return (c[1] is None or c[1] < v) and (c[2] is None or v < c[2])


def table_coords(t):
    """Each axis of a table as the answers read it: a number's coordinate from its bounds — equal ends
    one number, unequal ones the numbers strictly between — or the word a coordinate is written as."""
    out = []
    for a in t["axes"]:
        labels = a.get("coords") or []
        bs = a.get("bounds") or []
        cs = []
        for i, lab in enumerate(labels):
            b = bs[i] if i < len(bs) else None
            if b is None:
                cs.append(("w", lab))
            else:
                lo, hi = num(b[0]), num(b[1])
                cs.append(("x", lo) if lo is not None and lo == hi else ("b", lo, hi))
        out.append(cs)
    return out


def t_run(k, c):
    """The whole steps of 1/k a number's coordinate holds, an end None where it runs on."""
    if c[0] == "x":
        return (math.ceil(c[1] * k), math.floor(c[1] * k))
    return (None if c[1] is None else math.floor(c[1] * k) + 1, None if c[2] is None else math.ceil(c[2] * k) - 1)


def cv_holds(cv, v):
    if cv[0] == "val":
        return v == cv[1]
    if cv[0] == "coord":
        return wholds(cv[1], v)
    _, k, i, j = cv
    return isinstance(v, Fraction) and (v * k).denominator == 1 and i <= v * k <= j


def coord_inside(y, x):
    if y[0] == "x":
        return wholds(x, y[1])
    if x[0] != "b":
        return False
    return (x[1] is None or (y[1] is not None and x[1] <= y[1])) and (x[2] is None or (y[2] is not None and y[2] <= x[2]))


def coord_apart(y, x):
    if y[0] == "x":
        return not wholds(x, y[1])
    if x[0] == "x":
        return not wholds(y, x[1])
    return (y[2] is not None and x[1] is not None and y[2] <= x[1]) or (x[2] is not None and y[1] is not None and x[2] <= y[1])


def near_of(cs, cv):
    """The coordinates of a table's axis a cell may stand on."""
    out = []
    for idx, c in enumerate(cs):
        if cv[0] == "val":
            ok = wholds(c, cv[1])
        elif cv[0] == "coord" and cv[1][0] == "w":
            ok = c == cv[1]
        elif cv[0] == "coord":
            ok = c[0] != "w" and not coord_apart(cv[1], c)
        else:
            _, k, i, j = cv
            if c[0] == "w":
                ok = False
            else:
                a, b = t_run(k, c)
                ok = (a is None or a <= j) and (b is None or i <= b)
        if ok:
            out.append(idx)
    return out


def covers_from(j, cur, runs):
    for a, b in runs:
        if a is None or a <= cur:
            if b is None:
                return True
            cur = max(cur, b + 1)
    return j < cur


def covered(cs, cv):
    """Every value of the cell stands on some coordinate of the axis."""
    if cv[0] == "val":
        return any(wholds(c, cv[1]) for c in cs)
    if cv[0] == "coord" and cv[1][0] == "w":
        return cv[1] in cs
    if cv[0] == "coord":
        return any(c[0] != "w" and coord_inside(cv[1], c) for c in cs)
    _, k, i, j = cv
    return k > 0 and covers_from(j, i, [t_run(k, cs[c]) for c in near_of(cs, cv) if cs[c][0] != "w"])


def cv_single(cv):
    if cv[0] == "val":
        return cv[1]
    if cv[0] == "coord" and cv[1][0] == "w":
        return cv[1][1]
    if cv[0] == "coord" and cv[1][0] == "x":
        return cv[1][1]
    if cv[0] == "run" and cv[2] == cv[3] and cv[1] > 0:
        return Fraction(cv[2], cv[1])
    return None


def row_at(t, p):
    """The row of a table that answers at a point: under `first` the earliest whose box takes it,
    under `unique` the only one."""
    taking = [r for r in t["rows"] if all(c in r["accepts"][ai] for ai, c in enumerate(p))]
    if t["policy"] == "first":
        return taking[0] if taking else None
    return taking[0] if len(taking) == 1 else None


def out_of(t, r, col):
    """What a row writes into a column it decides: ("val", v), ("name", n), or None."""
    decides = t.get("decides") or []
    if col not in decides:
        return None
    oi = decides.index(col)
    prods = r.get("produces") or []
    if oi < len(prods) and prods[oi] is not None:
        return ("val", prods[oi])
    writes = r.get("writes") or []
    w = writes[oi] if oi < len(writes) else None
    e = (w or {}).get("expr") if isinstance(w, dict) else None
    if not isinstance(e, dict):
        return None
    if "name" in e:
        return ("name", e["name"])
    if "value" in e:
        return ("val", Fraction(e["value"]))
    return None


def decider_of(tables, col):
    return next((t for t in tables if col in (t.get("decides") or [])), None)


def cell_of(tables, coords, cell, fuel, n):
    """What a cell holds for a column: what the table that decides it answers there, one value at
    every point the cell may stand on — or what the cell holds for a column of the walk."""
    t = decider_of(tables, n)
    if t is None:
        return cell(n)
    if fuel == 0:
        return None
    nears = []
    for ai, a in enumerate(t["axes"]):
        cv = cell_of(tables, coords, cell, fuel - 1, a["column"])
        cs = coords[id(t)][ai]
        if cv is None or not covered(cs, cv):
            return None
        nears.append(near_of(cs, cv))
    vals = []
    for p in itertools.product(*nears):
        r = row_at(t, p)
        if r is None:
            return None
        o = out_of(t, r, n)
        if o is None:
            return None
        if o[0] == "val":
            vals.append(o[1])
        else:
            m = cell_of(tables, coords, cell, fuel - 1, o[1])
            v = None if m is None else cv_single(m)
            if v is None:
                return None
            vals.append(v)
    if not vals or any(v != vals[0] for v in vals[1:]):
        return None
    return ("val", vals[0])


def value_of(tables, coords, env, fuel, n):
    """The value of a column at a call: what the table that decides it answers, read where the values
    of its own columns stand on its axes; any other name the call's."""
    t = decider_of(tables, n)
    if t is None:
        return env(n)
    if fuel == 0:
        return None
    p = []
    for ai, a in enumerate(t["axes"]):
        v = value_of(tables, coords, env, fuel - 1, a["column"])
        idx = next((i for i, c in enumerate(coords[id(t)][ai]) if v is not None and wholds(c, v)), None)
        if idx is None:
            return None
        p.append(idx)
    r = row_at(t, p)
    if r is None:
        return None
    o = out_of(t, r, n)
    if o is None:
        return None
    if o[0] == "val":
        return o[1]
    return value_of(tables, coords, env, fuel - 1, o[1])


def keeps(rounds, col, v):
    """A value its output's rounding leaves as it is: a word, or a number on the grid it rounds to."""
    if isinstance(v, str):
        return True
    g = rounds.get(col)
    return g is not None and Fraction(g) > 0 and (v / Fraction(g)).denominator == 1


def fixed_tiles(cert, axis, cs):
    """An axis that is not random covers what its column takes: a number's coordinates tile its range
    on the grid the walk cuts it on (two points a step apart, or a point and an interval that begins or
    ends at it), after the absent value where it is optional; a word's are its words."""
    col = axis["column"]
    ty = str((cert.get("types") or {}).get(col, ""))
    base = ty[:-1] if ty.endswith("?") else ty
    enums = cert.get("enums") or {}
    if base == "bool" or base in enums:
        words = ["false", "true"] if base == "bool" else list((cert.get("enum_ranges") or {}).get(col) or enums[base])
        if ty.endswith("?"):
            words = ["none"] + words
        have = [c[1] for c in cs if c[0] == "w"]
        return all(w in have for w in words)
    if base == "string":
        return False
    xs = cs
    if ty.endswith("?"):
        if not xs or xs[0] != ("w", "none"):
            return False
        xs = xs[1:]
    st = num(axis.get("step"))
    r = (cert.get("ranges") or {}).get(col) or [None, None]
    lo, hi = num(r[0]), num(r[1])
    if not xs or st is None or st <= 0 or any(c[0] == "w" for c in xs):
        return False
    def on_grid(v):
        return (v / st).denominator == 1
    first, last = xs[0], xs[-1]
    if (lo is not None and first != ("x", lo)) or (lo is None and not (first[0] == "b" and first[1] is None)):
        return False
    if (hi is not None and last != ("x", hi)) or (hi is None and not (last[0] == "b" and last[2] is None)):
        return False
    for c in xs:
        ends = [c[1]] if c[0] == "x" else [e for e in (c[1], c[2]) if e is not None]
        if not all(on_grid(e) for e in ends):
            return False
        if c[0] == "b" and c[1] is not None and c[2] is not None and not c[1] < c[2]:
            return False
    for a, b in zip(xs, xs[1:]):
        if a[0] == "x" and b[0] == "x":
            ok = b[1] == a[1] + st
        elif a[0] == "x":
            ok = b[1] == a[1]
        elif b[0] == "x":
            ok = a[2] is not None and b[1] == a[2]
        else:
            ok = False
        if not ok:
            return False
    return True


def run_arith(where, k, holds):
    """A class whose points the check ran one by one, held to its own arithmetic: the chance it came
    to is its lowest and its highest, and both pass the claim — a claim is one value or comparisons
    with values, so what it lets in is an interval, and the two ends passing it is every chance between
    them passing it."""
    p, lo, hi = k.get("chance"), k.get("low"), k.get("high")
    if (lo is None) != (hi is None) or (p is not None and (lo != p or hi != p)):
        raise Bad(f"{where}: a class's chance {p} is not the lowest and the highest it came to")
    if lo is not None and Fraction(lo) > Fraction(hi):
        raise Bad(f"{where}: a class's lowest chance {lo} is above its highest {hi}")
    for q in (lo, hi):
        if q is not None and not holds(Fraction(q)):
            raise Bad(f"{where}: a class's chance {q} does not pass the claim")


def chance_of_text(text):
    """The chance a claim's cell writes, as the file writes it: a value (`5/14`, `0`, `35%`, `0.1%`),
    or comparisons with values (`>=1/4 <=1/2`). None where it is not one."""
    def value(t):
        t = t.strip()
        if "/" in t:
            a, b = t.split("/", 1)
            ka, kb = lit_key(a), lit_key(b)
            if ka[0] != "n" or kb[0] != "n" or ka[2] or kb[2] or kb[1] == 0 or ka[1].denominator != 1 or kb[1].denominator != 1:
                return None
            return ka[1] / kb[1]
        k = lit_key(t)
        if k[0] != "n":
            return None
        if k[2] == "%":
            return k[1] / 100
        return k[1] if not k[2] else None
    s = text.strip()
    if not any(s.startswith(o) for o in ("<", ">")):
        v = value(s)
        return None if v is None else {"is": v}
    out = []
    for op, lit in split_cmp(s) or []:
        v = value(lit)
        if v is None:
            return None
        out.append((op, v))
    return {"cmp": out} if out else None


def check_claim_cells(cert, path):
    """The claims of `chances` are the file's (rulec's §15.243): every cell of a claim and its chance
    are the cells of the line of the `chances` table they stand on, and the chance the file writes is
    the one the counts are held to. Returns how many cells were read back."""
    ch = cert.get("chances")
    if not ch:
        return 0
    with open(path, "rb") as fh:
        lines = fh.read().split(b"\n")
    read = 0
    for cl in ch.get("claims", []):
        where = f"chances {cl['chances'] + 1} row {cl['row']}"
        src = cl.get("source")
        if not isinstance(src, dict):
            STATED.append(f"{where}: where its cells stand, which the certificate does not say")
            continue
        spans = list(src.get("inputs") or []) + list(src.get("outputs") or []) + [src.get("chance")]
        if len(spans) != len(cl.get("inputs", [])) + len(cl.get("outputs", [])) + 1 or any(not isinstance(sp, dict) for sp in spans):
            raise Bad(f"{where}: the certificate names another number of cells than the claim has")
        ln = spans[0]["line"]
        line = lines[ln - 1] if 0 < ln <= len(lines) else b""
        if any(sp["line"] != ln for sp in spans) or fields_of(line) != [(sp["col"], sp["col"] + sp["len"]) for sp in spans]:
            raise Bad(f"{where}: its cells are not the cells of the line it is written on")
        for sp in spans:
            got = line[sp["col"]:sp["col"] + sp["len"]].decode("utf-8", "replace")
            if got != sp.get("text"):
                raise Bad(f"{where}: the file says `{got}` where the certificate quotes `{sp.get('text')}`")
            read += 1
        said = chance_of_text(src["chance"]["text"])
        want = cl.get("chance") or {}
        if said is None:
            raise Bad(f"{where}: the chance the file writes, `{src['chance']['text']}`, cannot be read as one")
        if "is" in want:
            same = "is" in said and said["is"] == Fraction(want["is"])
        else:
            same = "cmp" in said and [(op, v) for op, v in said["cmp"]] == [(op, Fraction(v)) for op, v in want.get("cmp", [])]
        if not same:
            raise Bad(f"{where}: the file writes the chance `{src['chance']['text']}`, and the certificate holds the counts to another")
    # The elements of the sequences a pick by weight walks are the file's (§15.243).
    for sq in (ch.get("weighted") or {}).get("sequences", []):
        for row in sq.get("rows", []):
            ln = row.get("line", 0)
            line = lines[ln - 1] if 0 < ln <= len(lines) else b""
            cells = row.get("cells", [])
            spans = [c.get("source") for c in cells]
            if any(not isinstance(sp, dict) or sp.get("line") != ln for sp in spans) or \
                    fields_of(line) != [(sp["col"], sp["col"] + sp["len"]) for sp in spans]:
                raise Bad(f"sequence {sq['name']}: the cells of a row are not the cells of the line it is written on")
            for c, sp in zip(cells, spans):
                got = line[sp["col"]:sp["col"] + sp["len"]].decode("utf-8", "replace")
                if got != sp.get("text"):
                    raise Bad(f"sequence {sq['name']}: the file says `{got}` where the certificate quotes `{sp.get('text')}`")
                v = c.get("value")
                k = lit_key(got)
                agrees = got == v or (k[0] == "n" and not k[2] and num(v) == k[1])
                if not agrees:
                    raise Bad(f"sequence {sq['name']}: the file writes `{got}`, and the certificate reads another value")
                read += 1
    return read


def check_weighted(cert, ch, cl, where, holds, tables, coords, fuel, rounds, draws, classes):
    """A claim over a pick by weight (rulec's §15.226), run again: at every draw the walk goes through the
    elements of the sequence the claim names, each element's verdict read off the tables, an arm that
    takes it putting its weight on the running total and taking its value the first time the draw is
    below it; the door turns away a draw at or past the total, unless the weights add up to 0, when the
    walk answers `exhausted`, and a walk over no element answers `empty`. Every value is worked out
    exactly. Returns how many draws were walked, or None where the claim is left stated."""
    w = ch["weighted"]
    types = cert.get("types") or {}
    def numeric(n):
        return str(types.get(n, "")).rstrip("?") not in ("bool", "string", "") and str(types.get(n, "")).rstrip("?") not in (cert.get("enums") or {})
    seq_name = next(c["cell"]["sequence"] for c in cl.get("inputs", []) if "sequence" in c["cell"])
    sq = next((q for q in w.get("sequences", []) if q["name"] == seq_name), None)
    if sq is None:
        raise Bad(f"{where}: the sequence {seq_name} the claim names is none the certificate carries")
    elems = []
    for row in sq.get("rows", []):
        e = {}
        for f, c in zip(sq.get("fields", []), row.get("cells", [])):
            v = c.get("value")
            e[f] = Fraction(v) if numeric(f) and v is not None else v
        elems.append(e)
    ins = [(c["column"], c["cell"]) for c in cl.get("inputs", []) if "sequence" not in c["cell"]]
    outs = [(c["column"], c["cell"]) for c in cl.get("outputs", [])]
    rins = [chance_rin(cert, n) for n, _ in ins]
    if any(x is None for x in rins):
        STATED.append(f"{where}: an input that is not random takes values this program does not list")
        return None
    lists = [[v for v in rin_grid(rin) if cell_passes(cell, v if isinstance(v, str) else str(v))] for (n, cell), rin in zip(ins, rins)]
    draw = w.get("draw")
    grid = rin_grid(draws[draw])
    arms = {a["is"]: a.get("take") for a in w.get("arms", [])}
    values = cert.get("values", [])
    walked = runs = 0
    seen = []
    for xs in itertools.product(*lists):
        base = dict(zip([n for n, _ in ins], xs))
        d = h = 0
        taking = None
        for r in grid:
            env = dict(base)
            env[draw] = r
            try:
                for v in values:
                    x = exact_value(v["expr"], env)
                    if x is not None and v.get("type") == "bool":
                        x = "true" if x == 1 else "false"
                    env[v["name"]] = x
                ts = []
                for e in elems:
                    ee = dict(env)
                    ee.update(e)
                    verdict = value_of(tables, coords, ee.get, fuel, w["verdict"])
                    if verdict not in arms:
                        raise Bad(f"{where}: an element's verdict {verdict} has no arm")
                    take = arms[verdict]
                    if take is None:
                        continue
                    val = exact_value(take["value"], ee)
                    wt = exact_value(take["weight"], ee)
                    if val is None or wt is None:
                        raise Bad(f"{where}: an element's value or weight cannot be worked out")
                    ts.append((val, wt))
                empty = exact_value(w["empty"], env)
                total = sum((wt for _, wt in ts), Fraction(0))
                if not elems:
                    door, ans = True, empty
                else:
                    door = total == 0 or r < total
                    if w.get("exhausted") is None:
                        acc, ans = Fraction(0), None
                        for val, wt in ts:
                            acc += wt
                            if r < acc:
                                ans = val
                                break
                        if ans is None:
                            ans = empty
                    else:
                        ans = exact_value(w["exhausted"], env)
            except Inexact:
                STATED.append(f"{where}: a value of the walk is rounded at a draw where the way it is rounded decides it")
                return None
            taking = len(ts) if taking is None else taking
            walked += 1
            if not door:
                continue
            env[w["answer"]] = ans
            hit = True
            for col, cell in outs:
                if "any" in cell:
                    continue
                val = value_of(tables, coords, env.get, fuel, col)
                if val is not None and not keeps(rounds, col, val):
                    STATED.append(f"{where}: an output is rounded at a draw where the way it is rounded decides it")
                    return None
                if not cell_passes(cell, val if val is None or isinstance(val, str) else str(val)):
                    hit = False
            d += 1
            h += 1 if hit else 0
        if d:
            runs += (taking or 0) + 1
            seen.append(Fraction(h, d))
            if not holds(Fraction(h, d)):
                raise Bad(f"{where}: at {base} the chance {Fraction(h, d)} does not pass the claim")
    k0 = classes[0] if classes else {}
    low, high = (min(seen), max(seen)) if seen else (None, None)
    if int(k0.get("run", -1)) != runs:
        raise Bad(f"{where}: the rule is run {runs} times here, and the certificate says {k0.get('run')}")
    if (num(k0.get("low")), num(k0.get("high"))) != (low, high):
        raise Bad(f"{where}: the chances of the draws come to {low}–{high}, and the certificate says {k0.get('low')}–{k0.get('high')}")
    return walked


def check_chances(cert, ch):
    """The claims of `chances` (rulec's §15.224–§15.226, §15.243), held to the rows. Each random input
    is drawn from the whole steps of its range or its words, and each random axis's coordinates tile
    them, side by side, none twice. The answer of each box is worked out again here from the tables'
    rows — at every point of each table the box may stand on, the row that answers writes one value —
    so the hits of a class are counted here, not read; the classes listed are every combination of the
    other axes' coordinates inside the claim's cells, and those axes cover what their columns take.
    A claim the check ran point by point is run again here, every value the rule computes worked out
    exactly. What is left stated: a claim undecided within the budget (W129), one over a sequence the
    rule walks, and a class the walk found no input in."""
    ranges, scales = cert.get("ranges", {}), cert.get("scales", {})
    types = cert.get("types", {})
    tables = cert.get("tables", [])
    coords = {id(t): table_coords(t) for t in tables}
    fuel = len(tables) + 1
    rounds = ch.get("rounds") or {}
    # What each random input is drawn from, held to the rule's ranges and types.
    draws = {}
    for r in ch.get("random", []):
        name = r["name"]
        rin = chance_rin(cert, name)
        if rin is None:
            raise Bad(f"chances: {name} is drawn, and its type {types.get(name)} is none this program draws")
        if rin[0] == "words":
            if list(r.get("values") or []) != rin[1]:
                raise Bad(f"chances: {name} is drawn from {r.get('values')}, and the rule's type gives {rin[1]}")
        else:
            lo, hi = ranges.get(name, [None, None])
            if [str(x) for x in r.get("range") or []] != [str(lo), str(hi)]:
                raise Bad(f"chances: {name} is drawn over {r.get('range')}, and its range is {[lo, hi]}")
            if int(r.get("scale", 0)) != rin[3]:
                raise Bad(f"chances: {name} is drawn on another grid than the scale it is stored at")
        if int(r["points"]) != rin_count(rin):
            raise Bad(f"chances: {name} has {rin_count(rin)} points, and the certificate says {r['points']}")
        draws[name] = rin
    axes = ch.get("axes", [])
    random_ix = [i for i, a in enumerate(axes) if a.get("random")]
    fixed_ix = [i for i, a in enumerate(axes) if not a.get("random")]
    if axes and sorted(axes[i]["column"] for i in random_ix) != sorted(draws):
        raise Bad("chances: the random axes are not the random inputs, each once")
    # Each random axis: its coordinates, each with the points it holds, side by side over the draws.
    rand = []
    for i in random_ix:
        a = axes[i]
        rin = draws[a["column"]]
        cs = []
        for co in a["coords"]:
            c = wcoord(co)
            if c is None:
                raise Bad(f"chances: a coordinate of {a['column']} is no number and no word")
            if rin[0] == "num":
                k = rin[3]
                if c[0] == "w" or (c[0] == "b" and (c[1] is None or c[2] is None)):
                    raise Bad(f"chances: a coordinate of {a['column']} runs on without an end")
                ij = t_run(k, c)
                cs.append((("run", k, ij[0], ij[1]), max(0, ij[1] - ij[0] + 1)))
            else:
                if c[0] != "w":
                    raise Bad(f"chances: a coordinate of {a['column']} is no word")
                drawn = c[1] in rin[1]
                cs.append((("coord", c), 1 if drawn else 0))
        if rin[0] == "num":
            _, lo, hi, k = rin
            cur = math.ceil(lo * k)
            for (cv, _n) in cs:
                if cv[2] != cur or cv[2] > cv[3] + 1:
                    raise Bad(f"chances: the coordinates of {a['column']} do not tile its range")
                cur = cv[3] + 1
            if cur != math.floor(hi * k) + 1:
                raise Bad(f"chances: the coordinates of {a['column']} do not run over its range")
        else:
            if [cv[1][1] for (cv, n) in cs if n] != rin[1]:
                raise Bad(f"chances: the words of {a['column']} are not the ones its input is drawn from, each once")
        rand.append(cs)
    # The axes that are not random cover what their columns take.
    fixed = []
    strings = False
    for i in fixed_ix:
        a = axes[i]
        cs = [wcoord(co) for co in a["coords"]]
        if any(c is None for c in cs) or str(types.get(a["column"], "")).rstrip("?") == "string":
            strings = True
            fixed.append(cs)
            continue
        if not fixed_tiles(cert, a, cs):
            raise Bad(f"chances: the coordinates of {a['column']} do not cover what it takes")
        # A claim counted box by box speaks for the values on the grid this axis is cut on, which has
        # to divide the step the column's values sit on (§15.190): cut coarser, a value the column
        # takes would lie off the grid, and the claim would be counted over fewer values than there are.
        ty = str(types.get(a["column"], ""))
        base = ty[:-1] if ty.endswith("?") else ty
        if base != "bool" and base not in (cert.get("enums") or {}):
            st, g = num(a.get("step")), column_grid(cert, a["column"])
            if g is None:
                STATED.append(f"the step chances' {a['column']} is cut at, which no scale here bears out")
            elif (g / st).denominator != 1:
                raise Bad(f"chances: {a['column']} is cut every {st}, and its values sit on a step of {g}, "
                          f"so a value can fall between two coordinates")
        fixed.append(cs)
    def fixed_cell(i, c):
        """What a coordinate of an axis that is not random holds: a number's whole steps of the walk's
        grid, where both its ends are there; otherwise every value of it."""
        a = axes[fixed_ix[i]]
        st = num(a.get("step"))
        if c[0] != "w" and st is not None and st > 0 and (1 / st).denominator == 1:
            k = int(1 / st)
            ij = t_run(k, c)
            if ij[0] is not None and ij[1] is not None:
                return ("run", k, ij[0], ij[1])
        return ("coord", c)
    values = cert.get("values", [])
    value_names = [v["name"] for v in values]
    inputs = list(cert.get("inputs") or [])
    walks = bool(cert.get("walks") or cert.get("fold_answers") or cert.get("extremes"))
    counted = run_total = proved = walked = 0
    for cl in ch.get("claims", []):
        where = f"chances {cl['chances'] + 1} row {cl['row']}"
        test = cl.get("chance") or {}
        def holds(p):
            if "is" in test:
                return p == Fraction(test["is"])
            return all({"<=": p <= Fraction(b), "<": p < Fraction(b), ">=": p >= Fraction(b), ">": p > Fraction(b)}[op]
                       for op, b in test.get("cmp", []))
        if cl.get("verdict") == "undecided":
            STATED.append(f"{where}: undecided within the budget (W129)")
            continue
        if cl.get("verdict") != "holds":
            raise Bad(f"{where}: the claim is {cl.get('verdict')}, and a rule whose claim does not hold has no certificate")
        ins = [(c["column"], c["cell"]) for c in cl.get("inputs", [])]
        outs = [(c["column"], c["cell"]) for c in cl.get("outputs", [])]
        sequence = any("sequence" in c for _, c in ins)
        classes = cl.get("classes", [])
        if not axes:
            # Run one by one: every combination the cells let in, every draw.
            if sequence and ch.get("weighted"):
                walked_here = check_weighted(cert, ch, cl, where, holds, tables, coords, fuel, rounds, draws, classes)
                if walked_here is None:
                    continue
                walked += walked_here
                proved += 1
                continue
            if walks or sequence:
                # What the check ran is not run again here; its lowest and highest chance still pass the claim.
                for k in classes:
                    run_arith(where, k, holds)
                STATED.append(f"{where}: its draws are counted over a sequence the rule walks, which this program does not walk")
                continue
            rins = [chance_rin(cert, n) for n, _ in ins]
            if any(x is None for x in rins):
                STATED.append(f"{where}: an input that is not random takes values this program does not list")
                continue
            # Each value reads only inputs, the columns tables decide, and the values listed before it.
            decided = {d for t in tables for d in (t.get("decides") or [])}
            def names(e):
                if not isinstance(e, dict):
                    return set()
                out = {e["name"]} if "name" in e else set()
                for k in ("l", "r"):
                    if k in e:
                        out |= names(e[k])
                for a in e.get("args", []) or []:
                    out |= names(a)
                return out
            if not all(m in inputs or m in decided or m in value_names[:i]
                       for i, v in enumerate(values) for m in names(v.get("expr"))):
                STATED.append(f"{where}: a value the rule computes reads one listed after it")
                continue
            lists = []
            for (n, cell), rin in zip(ins, rins):
                if rin[0] == "num" and "cmp" in cell:
                    _, lo, hi, k = rin
                    for op, b in cell["cmp"]:
                        b = Fraction(b)
                        if op in (">=", ">"):
                            lo = max(lo, b)
                        else:
                            hi = min(hi, b)
                    vals = [v for v in rin_grid(("num", lo, hi, k)) if cell_passes(cell, str(v))]
                else:
                    vals = [v for v in rin_grid(rin) if cell_passes(cell, v if isinstance(v, str) else str(v))]
                lists.append(vals)
            names_r = list(draws)
            grids = [rin_grid(draws[n]) for n in names_r]
            per = math.prod(len(g) for g in grids)
            if math.prod(len(l) for l in lists) * per > 4000000:
                STATED.append(f"{where}: its points are more than this program runs")
                continue
            cons = cert.get("constraints", [])
            def side(env, s):
                ns = s if isinstance(s, list) else [s]
                vs = [env.get(n) for n in ns]
                return None if any(not isinstance(v, Fraction) for v in vs) else sum(vs, Fraction(0))
            ran, seen, inexact = 0, [], False
            for xs in itertools.product(*lists):
                base = dict(zip([n for n, _ in ins], xs))
                d = h = 0
                for rs in itertools.product(*grids):
                    env = dict(base)
                    env.update(zip(names_r, rs))
                    ok_door = True
                    for k in cons:
                        a, b = side(env, k["left"]), side(env, k["right"])
                        if a is None or b is None or not {"<=": a <= b, "<": a < b, ">=": a >= b, ">": a > b}[k["op"]]:
                            ok_door = False
                            break
                    if not ok_door:
                        continue
                    try:
                        for v in values:
                            x = exact_value(v["expr"], env)
                            if x is not None and v.get("type") == "bool":
                                x = "true" if x == 1 else "false"
                            env[v["name"]] = x
                    except Inexact:
                        inexact = True
                        break
                    hit = True
                    for col, cell in outs:
                        if "any" in cell:
                            continue
                        val = value_of(tables, coords, env.get, fuel, col)
                        if val is not None and not keeps(rounds, col, val):
                            inexact = True
                            break
                        if not cell_passes(cell, val if val is None or isinstance(val, str) else str(val)):
                            hit = False
                    if inexact:
                        break
                    d += 1
                    h += 1 if hit else 0
                if inexact:
                    break
                if d:
                    ran += per
                    seen.append(Fraction(h, d))
                    if not holds(Fraction(h, d)):
                        raise Bad(f"{where}: at {base} the chance {Fraction(h, d)} does not pass {test}")
            if inexact:
                STATED.append(f"{where}: a value the rule computes, or an output, is rounded at a draw where the way it is rounded decides it")
                continue
            k0 = classes[0] if classes else {}
            low, high = (min(seen), max(seen)) if seen else (None, None)
            if int(k0.get("run", -1)) != ran:
                raise Bad(f"{where}: {ran} points are run here, and the certificate says {k0.get('run')}")
            if (num(k0.get("low")), num(k0.get("high"))) != (low, high):
                raise Bad(f"{where}: the chances run here come to {low}–{high}, and the certificate says {k0.get('low')}–{k0.get('high')}")
            run_total += ran
            proved += 1
            continue
        if strings:
            STATED.append(f"{where}: a column of the walk is one of strings, which this program does not cut")
            continue
        # The classes listed are every combination of the other axes' coordinates inside the row.
        cells = {c: cell for c, cell in ins}
        inside = set()
        for at in itertools.product(*[range(len(axes[i]["coords"])) for i in fixed_ix]):
            ok = True
            for i, ci in zip(fixed_ix, at):
                cell = cells.get(axes[i]["column"])
                if cell is not None and not coord_in_cell(axes[i], axes[i]["coords"][ci], cell):
                    ok = False
                    break
            if ok:
                inside.add(at)
        listed = {}
        for k in classes:
            if [i for i, _ in k["at"]] != fixed_ix:
                raise Bad(f"{where}: a class is not one coordinate of each axis that is not random")
            listed[tuple(ci for _, ci in k["at"])] = k
        if set(listed) != inside:
            raise Bad(f"{where}: the classes listed are not the ones the row's cells take ({len(listed)} listed, {len(inside)} taken)")
        unsure = 0
        for at, k in listed.items():
            if k.get("empty") or "run" in k:
                if "run" in k:
                    run_arith(where, k, holds)
                unsure += 1
                continue
            cls = {axes[i]["column"]: fixed_cell(q, fixed[q][ci]) for q, (i, ci) in enumerate(zip(fixed_ix, at))}
            stated_boxes = {tuple(ci for _, ci in b["at"]): b for b in k.get("boxes", [])}
            tot = hits = 0
            for bx in itertools.product(*[range(len(cs)) for cs in rand]):
                n = math.prod(rand[q][ci][1] for q, ci in enumerate(bx))
                if n == 0:
                    continue
                cellmap = dict(cls)
                for q, ci in enumerate(bx):
                    cellmap[axes[random_ix[q]]["column"]] = rand[q][ci][0]
                hit, outs_here = True, {}
                for col, cell in outs:
                    if "any" in cell:
                        continue
                    cv = cell_of(tables, coords, cellmap.get, fuel, col)
                    if cv is None or cv[0] != "val" or not keeps(rounds, col, cv[1]):
                        raise Bad(f"{where}: the rows do not give {col} one value in a box")
                    outs_here[col] = cv[1]
                    if not cell_passes(cell, cv[1] if isinstance(cv[1], str) else str(cv[1])):
                        hit = False
                tot += n
                hits += n if hit else 0
                b = stated_boxes.get(bx)
                if b is None:
                    raise Bad(f"{where}: a box of a class is not listed")
                if int(b["points"]) != n or bool(b.get("hit")) != hit:
                    raise Bad(f"{where}: a box holds {n} points{' that pass' if hit else ''}, and the certificate says {b['points']}{' that pass' if b.get('hit') else ''}")
                for col, v in outs_here.items():
                    if chance_value((b.get("outputs") or {}).get(col)) != v:
                        raise Bad(f"{where}: the rows answer {col} = {v} in a box, and the certificate says {(b.get('outputs') or {}).get(col)}")
            if len(stated_boxes) != sum(1 for bx in itertools.product(*[range(len(cs)) for cs in rand])
                                        if math.prod(rand[q][ci][1] for q, ci in enumerate(bx))):
                raise Bad(f"{where}: the boxes of a class do not tile the random axes")
            if tot != int(k["points"]) or hits != int(k["hits"]):
                raise Bad(f"{where}: the boxes add up to {hits} of {tot}, and the certificate says {k['hits']} of {k['points']}")
            if tot and not holds(Fraction(hits, tot)):
                raise Bad(f"{where}: the chance {Fraction(hits, tot)} does not pass {test}")
            counted += 1
        if unsure:
            STATED.append(f"{where}: {unsure} classes the walk found no input in, or ran one by one, which this program does not show")
        else:
            proved += 1
    n = len(ch.get("claims", []))
    walk_note = f", {walked} draws walked by weight" if walked else ""
    return (f"chances: {proved} of {n} claims re-checked — {counted} classes counted box by box over the rows, "
            f"{run_total} points run one by one{walk_note}")


def check_prerequisites(cert, p):
    """One `prerequisites` block (rulec's §15.222), held to what it claims: every value of its enum
    once — on `start`, in `requires`, or in `never` — and every name a value; `topological` the
    values each after every one it may need, so there is no cycle; `reached` each value outside
    `never` after one value of each of its groups, from `start`, so each one can be reached; and,
    where `ordered_by` names a table, each value's answer worked out again from that table's rows
    in `tables`, and never below a prerequisite's along an edge whose prerequisite can be had."""
    name, over = p["name"], p["over"]
    values = cert.get("enums", {}).get(over)
    if values is None:
        raise Bad(f"prerequisites {name}: {over} is not an enum of the certificate")
    start, req, never = list(p["start"]), p["requires"], list(p["never"])
    said = start + list(req) + never
    for v in said:
        if v not in values:
            raise Bad(f"prerequisites {name}: {v} is not a value of {over}")
    if sorted(said) != sorted(values) or len(set(said)) != len(said):
        raise Bad(f"prerequisites {name}: the values of {over} are not each said once (on start, in requires or in never)")
    for v, groups in req.items():
        if not groups or any(not g for g in groups):
            raise Bad(f"prerequisites {name}: {v} has an empty group")
        for g in groups:
            for x in g:
                if x not in values:
                    raise Bad(f"prerequisites {name}: {v} needs {x}, which is not a value of {over}")
    needs = {v: [x for g in req.get(v, []) for x in g] for v in values}
    topo = p.get("topological")
    if topo is None or sorted(topo) != sorted(values) or len(set(topo)) != len(topo):
        raise Bad(f"prerequisites {name}: `topological` is not the values of {over}, each once")
    place = {v: i for i, v in enumerate(topo)}
    for v in values:
        for x in needs[v]:
            if place[x] >= place[v]:
                raise Bad(f"prerequisites {name}: {v} needs {x}, which `topological` does not put before it")
    reached = list(p["reached"])
    if len(set(reached)) != len(reached):
        raise Bad(f"prerequisites {name}: `reached` names a value twice")
    if sorted(reached + never) != sorted(values):
        raise Bad(f"prerequisites {name}: `reached` and `never` are not the values of {over}")
    seen = set()
    for v in reached:
        if v not in start and not all(any(x in seen for x in g) for g in req.get(v, [[]])):
            raise Bad(f"prerequisites {name}: {v} is reached before a value of each group it needs")
        seen.add(v)
    said_order = ""
    ob = p.get("ordered_by")
    if ob:
        t = next((t for t in cert["tables"] if t["table"] == ob["table"]), None)
        if t is None or len(t["axes"]) != 1 or t["decides"] != [ob["column"]]:
            raise Bad(f"prerequisites {name}: {ob['table']} is no table of one column of {over} that decides {ob['column']}")
        coords = t["axes"][0]["coords"]
        if list(coords) != list(values):
            raise Bad(f"prerequisites {name}: the axis of {ob['table']} is not the values of {over}")
        if ob["kind"] not in ("enum", "number"):
            raise Bad(f"prerequisites {name}: an order of kind {ob['kind']!r} cannot be read")
        answer = {}
        for i, v in enumerate(coords):
            # the first row that takes the value — the only one, in a table whose rows are disjoint.
            # An enum's value is what the row produces; a number is what it writes (§15.196), read
            # back from the file in `check_cells`.
            row = next((r for r in t["rows"] if i in r["accepts"][0]), None)
            if row is None:
                continue
            if ob["kind"] == "enum":
                if row["produces"] and row["produces"][0] is not None:
                    answer[v] = row["produces"][0]
            else:
                w = (row.get("writes") or [None])[0]
                e = w.get("expr") if isinstance(w, dict) else None
                if not isinstance(e, dict) or "num" not in e or num(e.get("value")) is None:
                    raise Bad(f"prerequisites {name}: {ob['table']} writes something other than a number for {v}")
                answer[v] = num(e["value"])
        said = ob.get("values") or {}
        same = set(answer) == set(said) and all(
            answer[v] == said[v] if ob["kind"] == "enum" else answer[v] == Fraction(said[v]) for v in answer)
        if not same:
            raise Bad(f"prerequisites {name}: the answers of {ob['table']} are not the ones its rows write")
        if ob["kind"] == "enum":
            order = cert.get("enums", {}).get(ob["enum"])
            if order != ob["order"] or ob["enum"] not in cert.get("ordered", []):
                raise Bad(f"prerequisites {name}: {ob['enum']} is not an ordered enum of the certificate")
            if any(a not in order for a in answer.values()):
                raise Bad(f"prerequisites {name}: {ob['table']} writes a value that is not one of {ob['enum']}")
            key = lambda v: order.index(answer[v])
        else:
            key = lambda v: answer[v]
        edges = 0
        for v in values:
            for x in needs[v]:
                if x in never:
                    continue
                # A value a case can take, and the one it needs, each have the answer the table gives it.
                if v not in answer or x not in answer:
                    raise Bad(f"prerequisites {name}: {ob['table']} gives no answer for {v if v not in answer else x}")
                edges += 1
                if key(v) < key(x):
                    raise Bad(f"prerequisites {name}: {ob['table']} puts {v} at {answer[v]}, before its prerequisite {x} at {answer[x]}")
        said_order = f", and {ob['table']} keeps its order along {edges} edges"
    n_edges = sum(len(needs[v]) for v in values)
    return (f"prerequisites {name} over {over}: {len(values)} values and {n_edges} edges with no cycle, "
            f"{len(reached)} reached from start and {len(never)} no case can take{said_order}")


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
                # The shape this program reads is version 1. A certificate of another shape
                # says so in `v`, and is refused rather than read as something it is not.
                if cert.get("v", 1) != 1:
                    raise ValueError(f"its format version is {cert['v']!r}; this program reads version 1")
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
                    if cert.get("machine") and cert["machine"].get("table"):
                        try:
                            lines.append("  " + check_machine_cells(cert, rule))
                        except Bad as e:
                            lines.append(f"  FAILED {e}")
                            ok = False
                    try:
                        claimed = check_claim_cells(cert, rule)
                    except Bad as e:
                        lines.append(f"  FAILED {e}")
                        ok = False
                        claimed = 0
                    try:
                        read, loose, apart = check_cells(cert, rule)
                        read += claimed
                        rest = f", {loose} literals not pinned to a boundary" if loose else ""
                        rest += f", {apart} rows written in an applied rule" if apart else ""
                        if loose:
                            STATED.append(f"{loose} literals written in a unit this program cannot read")
                        if apart:
                            STATED.append(f"{apart} rows written in an applied rule, and read back in its own certificate")
                        # Rows a contribution adds are read back out of its own file (§15.231).
                        others = sorted({r["file"]["path"] for t in cert.get("tables", []) for r in t.get("rows", []) if r.get("file")})
                        if others:
                            files = "file" if len(others) == 1 else "files"
                            lines.append(f"  the files say the same: {read} cells read back from {rule} and the {len(others)} {files} of its contributions{rest}")
                        else:
                            lines.append(f"  the file says the same: {read} cells read back from it{rest}")
                    except Bad as e:
                        lines.append(f"  FAILED {e}")
                        ok = False
                else:
                    lines.append(f"  FAILED the certificate is about another text than {rule}")
                    ok = False
                # Every contribution merged into the rule is the very file the certificate names (§15.243):
                # held to its SHA-256 whether it adds a row or not, and every row a contribution adds and
                # every term it puts into an open value is one of them.
                contribs = cert.get("contributions") or []
                held_files = 0
                for k in contribs:
                    there = os.path.join(os.path.dirname(rule), k.get("file", ""))
                    try:
                        with open(there, "rb") as fh:
                            digest = hashlib.sha256(fh.read()).hexdigest()
                    except OSError:
                        lines.append(f"  FAILED contribution {k.get('name')} is named as {k.get('file')}, which cannot be read")
                        ok = False
                        continue
                    if digest != k.get("sha256"):
                        lines.append(f"  FAILED contribution {k.get('name')} is read from another text than {there}")
                        ok = False
                    else:
                        held_files += 1
                if contribs:
                    named = {k.get("file") for k in contribs}
                    for t in cert.get("tables", []):
                        for r in t.get("rows", []):
                            if row_file(r) and row_file(r) not in named:
                                lines.append(f"  FAILED {t['table']}: row {r['row']} is written in {row_file(r)}, which is no contribution the certificate names")
                                ok = False
                    for o in cert.get("opens", []):
                        for term in o.get("terms", []):
                            fr = term.get("from")
                            if isinstance(fr, dict) and fr.get("file") not in named:
                                lines.append(f"  FAILED {o['name']}: a term comes from {fr.get('file')}, which is no contribution the certificate names")
                                ok = False
                    lines.append(f"  the digests of the {held_files} contributions merged into it are their files'")
                # Each contract's reading is tied to its text the same way (§15.142): the file
                # is named relative to the rule, and its digest is in the certificate.
                for k in cert.get("contracts", []):
                    there = os.path.join(os.path.dirname(rule), k["file"])
                    try:
                        with open(there, "rb") as fh:
                            digest = hashlib.sha256(fh.read()).hexdigest()
                    except OSError:
                        STATED.append(f"contract {k['shape']} is held to no text: {there} cannot be read")
                        continue
                    if digest == k.get("sha256"):
                        lines.append(f"  the digest of contract {k['shape']} is {there}'s")
                    else:
                        lines.append(f"  FAILED contract {k['shape']} is read from another text than {there}")
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
