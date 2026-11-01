#!/usr/bin/env python3
"""Write src/xid.rs from the Unicode Character Database (tools/xid/README.md).

    python3 tools/xid/make_table.py <DerivedCoreProperties.txt> <DerivedGeneralCategory.txt>

A name of ritsu's languages begins with a letter (Alphabetic) or `_`, and goes on with letters,
numbers (the general categories Nd, Nl and No) and `_`. A name of Rust's or of Python's begins
with XID_Start or `_`, and goes on with XID_Continue; one of TypeScript's begins with ID_Start or
`_`, and goes on with ID_Continue; one of Go's begins with a letter (the general categories Lu,
Ll, Lt, Lm and Lo) or `_`, and goes on with letters, decimal digits (Nd) and `_`. For each of the
three, the script writes the characters of the first kind that are not of the target's, as two
tables of ranges: those a name of the target does not begin with, and those it does not hold
after its first. chobo's E063 and its Rust clients read the tables of Rust, and kaname the tables
of the target it writes. The files are not kept in the repository; tools/xid/README.md says
where to get them, and the script refuses any but the two it was made from (their SHA-256).
"""
import hashlib
import sys

VERSION = (17, 0, 0)
FILES = [
    ("https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt",
     "24c7fed1195c482faaefd5c1e7eb821c5ee1fb6de07ecdbaa64b56a99da22c08"),
    ("https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt",
     "d62e5bab70ca74f099343f71224fa051cb1fdd61a1ab45c0488c44cfc0b6102e"),
]
UNDERSCORE = 0x5F


def read(path, sha256):
    raw = open(path, "rb").read()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != sha256:
        sys.exit(f"{path}: the SHA-256 is {digest}, not {sha256}: it is another version. "
                 "Read what changed, then update VERSION and FILES here and tools/xid/README.md.")
    return raw.decode("utf-8")


def values(text, wanted):
    """The code points of each value in `wanted` (a property or a general category), and the
    file's date line."""
    out = {w: set() for w in wanted}
    date = None
    for line in text.splitlines():
        if line.startswith("# Date:"):
            date = line.split(":", 1)[1].strip()
        fields = [f.strip() for f in line.split("#", 1)[0].split(";")]
        if len(fields) != 2 or fields[1] not in out:
            continue
        lo, _, hi = fields[0].partition("..")
        out[fields[1]].update(range(int(lo, 16), int(hi or lo, 16) + 1))
    return out, date


def ranges(points):
    out = []
    for u in sorted(points):
        if out and out[-1][1] == u - 1:
            out[-1][1] = u
        else:
            out.append([u, u])
    return out


def table(name, doc, points):
    rs = ranges(points)
    lines = [f"/// {d}" for d in doc]
    lines.append(f"pub const {name}: &[(u32, u32)] = &[")
    for i in range(0, len(rs), 4):
        lines.append("    " + " ".join(f"(0x{lo:04X}, 0x{hi:04X})," for lo, hi in rs[i:i + 4]))
    lines.append("];")
    return lines, len(rs)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    (core_url, core_sha), (gc_url, gc_sha) = FILES
    core, core_date = values(read(sys.argv[1], core_sha), ["Alphabetic", "XID_Start", "XID_Continue", "ID_Start", "ID_Continue"])
    gc, gc_date = values(read(sys.argv[2], gc_sha), ["Lu", "Ll", "Lt", "Lm", "Lo", "Nd", "Nl", "No"])
    letter = gc["Lu"] | gc["Ll"] | gc["Lt"] | gc["Lm"] | gc["Lo"]
    numeric = gc["Nd"] | gc["Nl"] | gc["No"]
    first = core["Alphabetic"] | {UNDERSCORE}
    after = core["Alphabetic"] | numeric | {UNDERSCORE}
    targets = [
        # the tables' prefix, whose names they are, what those begin with and what they hold after
        ("", "Rust's and Python's", "XID_Start", "XID_Continue",
         first - core["XID_Start"] - {UNDERSCORE}, after - core["XID_Continue"] - {UNDERSCORE}),
        ("ID_", "TypeScript's", "ID_Start", "ID_Continue",
         first - core["ID_Start"] - {UNDERSCORE}, after - core["ID_Continue"] - {UNDERSCORE}),
        ("GO_", "Go's", "a letter (Lu, Ll, Lt, Lm, Lo)", "a letter or a decimal digit (Nd)",
         first - letter - {UNDERSCORE}, after - letter - gc["Nd"] - {UNDERSCORE}),
    ]
    v = ".".join(str(n) for n in VERSION)
    tables, counts = [], []
    for prefix, whose, begins, holds, not_first, not_after in targets:
        first_lines, first_ranges = table(f"{prefix}NOT_FIRST", [
            f"The letters a name of ritsu's languages may begin with that a name of {whose} may not:",
            f"not {begins}.",
        ], not_first)
        after_lines, after_ranges = table(f"{prefix}NOT_AFTER", [
            "The letters and numbers a name of ritsu's languages may hold after its first that a name",
            f"of {whose} may not: not {holds}.",
        ], not_after)
        tables += ["", *first_lines, "", *after_lines]
        counts.append(f"{whose}: {len(not_first)} characters in {first_ranges} ranges begin none, {len(not_after)} in {after_ranges} are in none")
    lines = [
        "//! The characters a name of ritsu's languages may hold that a name of a target may not. A name of",
        "//! ritsu's languages takes a letter (Alphabetic) or `_` first, then letters, numbers (Nd, Nl, No)",
        "//! and `_`. Rust and Python take XID_Start or `_` first, then XID_Continue; TypeScript ID_Start or",
        "//! `_`, then ID_Continue; Go a letter (Lu, Ll, Lt, Lm, Lo) or `_`, then letters, decimal digits",
        "//! (Nd) and `_`. chobo's E063 and its Rust clients read the tables of Rust, and kaname the tables",
        "//! of the target it writes ([`crate::ident::rust_takes`] and the three beside it). Unicode 17.0 is",
        "//! the version of the identifiers of rustc 1.94 to 1.97, and of their `char`.",
        "//!",
        "//! Written by tools/xid/make_table.py; do not edit. The files it was made from, of the Unicode",
        f"//! Character Database {v}:",
        f"//!   {core_url}",
        f"//!   Date: {core_date}",
        f"//!   SHA-256: {core_sha}",
        f"//!   {gc_url}",
        f"//!   Date: {gc_date}",
        f"//!   SHA-256: {gc_sha}",
        *[f"//!   {c}." for c in counts],
        "//! The Unicode Character Database is Copyright © Unicode, Inc., under the Unicode License v3",
        "//! (THIRD_PARTY_NOTICES).",
        "",
        "/// The version of the Unicode Character Database the tables are made from.",
        f"pub const UNICODE: (u8, u8, u8) = ({VERSION[0]}, {VERSION[1]}, {VERSION[2]});",
        *tables,
    ]
    out = "src/xid.rs"
    with open(out, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{out}: Unicode {v}")
    for c in counts:
        print(f"  {c}")


main()
