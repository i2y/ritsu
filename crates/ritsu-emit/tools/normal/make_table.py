#!/usr/bin/env python3
"""Write src/normal_table.rs from the Unicode Character Database (chobo's DESIGN 18).

    python3 tools/normal/make_table.py <UnicodeData.txt> <DerivedNormalizationProps.txt>

Rust compares identifiers in NFC, and Python in NFKC, so two names of a book can be one identifier
in a client though chobo keeps them apart (its W108). The tables hold what the two forms need: each
character's decomposition of one step, with whether it is a compatibility one, the canonical
combining classes, and the pairs that compose. The files are not kept in the repository; they are
at https://www.unicode.org/Public/17.0.0/ucd/, and the script refuses any but the two it was made
from (their SHA-256).
"""
import hashlib
import sys

VERSION = (17, 0, 0)
SHA = {
    "UnicodeData.txt": "2e1efc1dcb59c575eedf5ccae60f95229f706ee6d031835247d843c11d96470c",
    "DerivedNormalizationProps.txt": "71fd6a206a2c0cdd41feb6b7f656aa31091db45e9cedc926985d718397f9e488",
}


def read(path, name):
    data = open(path, "rb").read()
    if hashlib.sha256(data).hexdigest() != SHA[name]:
        sys.exit(f"{path} is not the {name} of Unicode {VERSION}")
    return data.decode("utf-8")


def rows(values, per_line, render):
    out = []
    for i in range(0, len(values), per_line):
        out.append("    " + " ".join(render(v) for v in values[i : i + per_line]))
    return "\n".join(out)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    unicode_data = read(sys.argv[1], "UnicodeData.txt")
    props = read(sys.argv[2], "DerivedNormalizationProps.txt")
    excluded = set()
    for line in props.splitlines():
        line = line.split("#")[0].strip()
        if not line:
            continue
        fields = [f.strip() for f in line.split(";")]
        if len(fields) == 2 and fields[1] == "Full_Composition_Exclusion":
            lo, _, hi = fields[0].partition("..")
            excluded.update(range(int(lo, 16), int(hi or lo, 16) + 1))
    decompositions, data, combining, compositions = [], [], [], []
    for line in unicode_data.splitlines():
        f = line.split(";")
        code, ccc, dec = int(f[0], 16), int(f[3]), f[5]
        if ccc:
            if combining and combining[-1][1] == code - 1 and combining[-1][2] == ccc:
                combining[-1] = (combining[-1][0], code, ccc)
            else:
                combining.append((code, code, ccc))
        if not dec:
            continue
        compat = dec.startswith("<")
        parts = [int(x, 16) for x in dec.split(">")[-1].split()]
        decompositions.append((code, (len(data) << 6) | (len(parts) << 1) | int(compat)))
        data.extend(parts)
        if not compat and len(parts) == 2 and code not in excluded:
            compositions.append((parts[0], parts[1], code))
    compositions.sort()
    name = lambda v: f"0x{v:04X},"
    pair = lambda v: f"(0x{v[0]:04X}, 0x{v[1]:X}),"
    triple = lambda v: f"(0x{v[0]:04X}, 0x{v[1]:04X}, {v[2]}),"
    compose = lambda v: f"(0x{v[0]:04X}, 0x{v[1]:04X}, 0x{v[2]:04X}),"
    sha = "\n".join(f"//!   {k}, SHA-256: {v}" for k, v in SHA.items())
    print(f"""//! What NFC and NFKC need of the Unicode Character Database (src/normal.rs).
//!
//! Written by tools/normal/make_table.py; do not edit. The files it was made from, of the Unicode
//! Character Database {".".join(map(str, VERSION))} (https://www.unicode.org/Public/{".".join(map(str, VERSION))}/ucd/):
{sha}
//! The Unicode Character Database is Copyright © Unicode, Inc., under the Unicode License v3
//! (THIRD_PARTY_NOTICES).

/// The version of the Unicode Character Database the tables are made from.
pub const UNICODE: (u8, u8, u8) = {VERSION};

/// Each character with a decomposition, in order: the character, and where the characters of one
/// step of it are in [`DECOMPOSED`] (`offset << 6 | length << 1 | compatibility`).
#[rustfmt::skip]
pub const DECOMPOSITIONS: &[(u32, u32)] = &[
{rows(decompositions, 5, pair)}
];

/// The characters of the decompositions.
#[rustfmt::skip]
pub const DECOMPOSED: &[u32] = &[
{rows(data, 12, name)}
];

/// The canonical combining classes that are not 0, as ranges in order: first, last, class.
#[rustfmt::skip]
pub const COMBINING: &[(u32, u32, u8)] = &[
{rows(combining, 5, triple)}
];

/// The pairs that compose, in order: first, second, what they make. A decomposition NFC does not
/// compose back (Full_Composition_Exclusion) is left out.
#[rustfmt::skip]
pub const COMPOSITIONS: &[(u32, u32, u32)] = &[
{rows(compositions, 4, compose)}
];""")


main()
