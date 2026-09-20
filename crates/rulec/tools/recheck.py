#!/usr/bin/env python3
"""Re-check a rulec certificate, without rulec.

`rulec certificate <file.rule>` prints the evidence behind two of the five things `check`
proves. This program holds it to the claims, and shares no code with the tool that wrote
it — that is the whole point. It reads one certificate on stdin, or the files named as
arguments, and says for each table what it re-checked.

    $ rulec certificate rules/送料.rule | python3 tools/recheck.py
    送料料金: unique, 8 rows — 28 pairs disjoint, 8 rows reached                       ok

What it checks, per rule:

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

What it does **not** check: the units, a leaf that rests on an upstream table (re-checking
one needs that table's own region, and the summary says how many there were), and — before
all of them — that the table stated here is the table in the `.rule` file. The certificate
states its own universe; comparing that with the rule is what `rulec doc` is for.

Exit code 0 when every table holds, 1 when one does not, 2 on a certificate it cannot
read. No dependencies; Python 3.9 or later.
"""

import json
import math
import sys
from fractions import Fraction


class Bad(Exception):
    """A claim the certificate makes that does not hold."""


I64_MAX = 2**63 - 1


def num(x):
    """A rational as the certificate writes it: "7/2", "3", or null for an open end."""
    return None if x is None else Fraction(x)


def interval(e, ranges):
    """The interval an expression is forced into by the declared ranges. None where the
    certificate's own arithmetic gives up too (an unbounded name, a divisor that spans
    zero); the claim is then not re-checked here and the caller says so."""
    if "name" in e:
        lo, hi = ranges.get(e["name"], (None, None))
        return None if lo is None or hi is None else (num(lo), num(hi))
    if "num" in e:
        v = num(e.get("value"))
        return None if v is None else (v, v)
    if "op" in e:
        a, b = interval(e["l"], ranges), interval(e["r"], ranges)
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
    if "call" in e:
        args = e.get("args", [])
        inner = interval(args[0], ranges) if args else None
        if inner is None:
            return None
        if e["call"] in ("min", "max") and len(args) == 2:
            other = interval(args[1], ranges)
            if other is None:
                return None
            f = min if e["call"] == "min" else max
            return (f(inner[0], other[0]), f(inner[1], other[1]))
        # A rounding lands on the grid: the low end toward zero, the high end away from it,
        # whichever way the mode itself rounds.
        grid = interval(args[1], ranges) if len(args) > 1 else None
        if grid is None or grid[0] != grid[1] or grid[0] == 0:
            return None
        g = abs(grid[0])
        sign = lambda v: -1 if v < 0 else 1
        toward = lambda v: sign(v) * math.floor(abs(v) / g) * g
        away = lambda v: sign(v) * math.ceil(abs(v) / g) * g
        return (toward(inner[0]), away(inner[1]))
    return None


def check_values(cert):
    """int64: recompute each value's interval and hold the integer it stores to i64."""
    ranges = {k: (v[0], v[1]) for k, v in cert.get("ranges", {}).items()}
    checked = skipped = 0
    for v in cert.get("values", []):
        got = interval(v["expr"], ranges)
        if got is None:
            skipped += 1
            continue
        said = (num(v["interval"][0]), num(v["interval"][1]))
        if got[0] < said[0] or got[1] > said[1]:
            raise Bad(f"{v['name']}: the interval here is [{got[0]}, {got[1]}], wider than the stated [{said[0]}, {said[1]}]")
        stored = max(abs(got[0]), abs(got[1])) * v["scale"]
        if int(stored) > I64_MAX:
            raise Bad(f"{v['name']}: stores up to {int(stored)}, past int64")
        if int(stored) > int(Fraction(v["stored_max"])):
            raise Bad(f"{v['name']}: stores up to {int(stored)}, more than the stated {v['stored_max']}")
        checked += 1
    return checked, skipped


def check_cover(t):
    """Completeness: the cover has to tile the space, and every leaf has to hold."""
    axes, rows = t["axes"], {r["row"]: r for r in t["rows"]}
    if t.get("cover") is None:
        return None
    seen = {"rows": 0, "constraint": 0, "derived": 0, "upstream": 0}

    def bound_at(ai, ci):
        b = axes[ai].get("bounds", [None] * len(axes[ai]["coords"]))[ci]
        return (None, None) if b is None else (num(b[0]), num(b[1]))

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
            k = t["constraints"][node["constraint"]]
            col = [a["column"] for a in axes]
            if k["left"] not in col or k["right"] not in col:
                raise Bad(f"{t['table']}: the constraint {k['left']} {k['op']} {k['right']} is not on this table's axes")
            (ll, lh) = bound_at(col.index(k["left"]), path[col.index(k["left"])])
            (rl, rh) = bound_at(col.index(k["right"]), path[col.index(k["right"])])
            ok = {
                "<=": ll is not None and rh is not None and ll > rh,
                "<": ll is not None and rh is not None and ll >= rh,
                ">=": lh is not None and rl is not None and lh < rl,
                ">": lh is not None and rl is not None and lh <= rl,
            }[k["op"]]
            if not ok:
                raise Bad(f"{t['table']}: {k['left']} {k['op']} {k['right']} can hold here, so the box is not impossible")
            seen["constraint"] += 1
            return
        if "derived_axis" in node:
            ai = node["derived_axis"]
            if ai >= len(path):
                raise Bad(f"{t['table']}: a leaf points at axis {ai}, which this box has not fixed")
            lo, hi = bound_at(ai, path[ai])
            reach = t.get("_reach_of", {}).get(axes[ai]["column"])
            if reach is None:
                raise Bad(f"{t['table']}: {axes[ai]['column']} is called out of reach, but no expression for it is in the certificate")
            if not ((hi is not None and hi < reach[0]) or (lo is not None and lo > reach[1])):
                raise Bad(f"{t['table']}: {axes[ai]['column']} can reach this box, so it is not impossible")
            seen["derived"] += 1
            return
        if "upstream" in node:
            seen["upstream"] += 1
            return
        raise Bad(f"{t['table']}: a leaf of the cover has no kind this program knows")

    walk(t["cover"], [])
    return seen


def check_table(t):
    """Re-check one table. Returns a one-line summary; raises Bad on a claim that fails."""
    name = t["table"]
    axes = t["axes"]
    rows = {r["row"]: r for r in t["rows"]}
    if not rows:
        raise Bad(f"{name}: no rows")
    for r in t["rows"]:
        if len(r["accepts"]) != len(axes):
            raise Bad(f"{name}: row {r['row']} states {len(r['accepts'])} axes, the table has {len(axes)}")
        for ai, coords in enumerate(r["accepts"]):
            n = len(axes[ai]["coords"])
            if any(not isinstance(c, int) or c < 0 or c >= n for c in coords):
                raise Bad(f"{name}: row {r['row']} names a coordinate axis {ai} does not have")

    # (1) No two rows of a `unique` table meet.
    pairs = 0
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
        undecided = {(u["a"], u["b"]) for u in t["undecided"]}
        numbers = sorted(rows)
        for i, a in enumerate(numbers):
            for b in numbers[i + 1:]:
                if (a, b) not in told and (a, b) not in undecided:
                    raise Bad(f"{name}: rows {a} and {b} are neither proved apart nor listed as undecided")
    elif t["disjoint"]:
        raise Bad(f"{name}: a `first` table cannot claim its rows are disjoint")

    # (2) Every row is reached.
    reached = set()
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
        if t["policy"] == "first":
            for earlier in sorted(n for n in rows if n < row):
                if all(c in rows[earlier]["accepts"][ai] for ai, c in enumerate(at)):
                    raise Bad(f"{name}: the point for row {row} is taken by row {earlier} first")
        reached.add(row)
    unused = set(t.get("unused", []))
    for row in sorted(unused):
        if row in reached:
            raise Bad(f"{name}: row {row} is called unused and reached at the same time")
        if row not in rows:
            raise Bad(f"{name}: row {row} is not in the table")
    missing = sorted(set(rows) - reached - unused)
    if missing:
        raise Bad(f"{name}: no point is given for row(s) {missing}")

    # (3) The table is complete.
    seen = check_cover(t)
    if seen is None:
        cover_note = ", cover not stated"
    else:
        cover_note = f", {seen['rows']} boxes covered"
        for k, word in (("constraint", "by a constraint"), ("derived", "out of a derive's reach")):
            if seen[k]:
                cover_note += f" + {seen[k]} impossible {word}"
        if seen["upstream"]:
            cover_note += f" + {seen['upstream']} on an upstream claim this program does not re-check"

    kinds = {a["kind"] for a in axes}
    note = "" if kinds == {"input"} else f" (columns: {', '.join(sorted(kinds))})"
    unused_note = f", {len(unused)} unused" if unused else ""
    return (f"{name}: {t['policy']}, {len(rows)} rows — {pairs} pairs disjoint, "
            f"{len(reached)} rows reached{unused_note}{cover_note}{note}")


def check(cert):
    """Re-check one rule's certificate. Returns (lines, ok)."""
    out = [f"{cert['rule']} ({cert['alias']} v{cert['version']}, sha256:{cert['source_sha256'][:12]}) "
           f"— certificate by rulec {cert['rulec']}"]
    ok = True
    ranges = {k: (v[0], v[1]) for k, v in cert.get("ranges", {}).items()}
    try:
        checked, skipped = check_values(cert)
        rest = f", {skipped} not re-checkable here" if skipped else ""
        out.append(f"  int64: {checked} values fit{rest}")
    except Bad as e:
        out.append(f"  FAILED int64: {e}")
        ok = False
    # The reach of a derived value is the interval its own expression is forced into — the
    # cover's "out of reach" leaves are held to that, recomputed here.
    reach = {}
    for v in cert.get("values", []):
        got = interval(v["expr"], ranges)
        if got is not None:
            reach[v["name"]] = got
    if not cert["tables"]:
        out.append("  no table states a certificate")
    for t in cert["tables"]:
        t["_reach_of"] = reach
        try:
            out.append("  " + check_table(t))
        except Bad as e:
            out.append(f"  FAILED {e}")
            ok = False
    return out, ok


def main(argv):
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
            print("\n".join(lines))
            if not ok:
                worst = max(worst, 1)
    return worst


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
