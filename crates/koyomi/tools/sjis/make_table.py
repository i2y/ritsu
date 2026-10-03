#!/usr/bin/env python3
"""Write src/sjis_table.rs from the WHATWG index jis0208 (DESIGN 1.5, PLAN B.5).

    python3 tools/sjis/make_table.py <index-jis0208.txt>

The index is the table browsers read Shift_JIS pages with. This script checks that the file
is the version koyomi was built from (its SHA-256), then writes one string: for every pointer
0..11103, the character the index gives it, or U+FFFF where it gives none. The index itself is
not kept in the repository; tools/sjis/README.md says where to get it.
"""
import hashlib
import sys

URL = "https://encoding.spec.whatwg.org/index-jis0208.txt"
SHA256 = "341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468"
POINTERS = 11104
NONE = 0xFFFF
BACKSLASH = chr(92)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    raw = open(sys.argv[1], "rb").read()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != SHA256:
        sys.exit(f"the index's SHA-256 is {digest}, not {SHA256}: it was updated. "
                 "Read what changed, then update SHA256 here and the identifier and date in DESIGN.md 1.5.")
    identifier = date = None
    table = [NONE] * POINTERS
    count = 0
    for line in raw.decode("utf-8").splitlines():
        if line.startswith("# Identifier:"):
            identifier = line.split(":", 1)[1].strip()
        elif line.startswith("# Date:"):
            date = line.split(":", 1)[1].strip()
        elif line.strip() and not line.startswith("#"):
            pointer, code = line.split("\t")[:2]
            table[int(pointer)] = int(code, 16)
            count += 1
    chars = "".join(chr(c) for c in table)
    size = len(chars.encode("utf-8"))

    def literal(chunk):
        out = []
        for ch in chunk:
            if ch in ('"', BACKSLASH) or ch.isspace() or ord(ch) == NONE:
                out.append(BACKSLASH + "u{%x}" % ord(ch))
            else:
                out.append(ch)
        return '"' + "".join(out) + '"'

    lines = [
        "//! The Shift_JIS table: for every pointer 0..11103 of the WHATWG index jis0208, the",
        "//! character it decodes to, and U+FFFF where the index has none (DESIGN 1.5).",
        "//!",
        "//! Written by tools/sjis/make_table.py; do not edit. The index it was made from:",
        f"//!   {URL}",
        f"//!   Identifier: {identifier}",
        f"//!   Date: {date}",
        f"//!   SHA-256: {SHA256}",
        f"//!   {count} pointers; the string is {size} bytes of UTF-8.",
        "//! The index is part of the WHATWG Encoding Standard, Copyright © WHATWG (Apple, Google,",
        "//! Mozilla, Microsoft), licensed under CC BY 4.0; a portion of it incorporated into source",
        "//! code, as this table is, is under the BSD 3-Clause License instead (THIRD_PARTY_NOTICES.md).",
        "",
        f"pub const POINTERS: usize = {POINTERS};",
        f"pub const ASSIGNED: usize = {count};",
        "",
        "pub const TABLE: &str = concat!(",
    ]
    for i in range(0, POINTERS, 48):
        lines.append("    " + literal(chars[i:i + 48]) + ",")
    lines.append(");")
    sys.stdout.reconfigure(encoding="utf-8")
    out = "src/sjis_table.rs"
    with open(out, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{out}: {count} pointers, {size} bytes of UTF-8 (identifier {identifier}, {date})")


main()
