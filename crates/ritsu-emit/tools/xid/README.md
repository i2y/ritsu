# The tables of the characters of the targets' names

`src/xid.rs` is generated from two files of the Unicode Character Database 17.0.0, the version
of the identifiers of rustc 1.94 to 1.100. A name of ritsu's languages takes a letter (Alphabetic)
or `_` first, then letters, numbers (Nd, Nl, No) and `_`. The file holds the letters and the
numbers of 17.0 (`LETTERS`, `NUMBERS`), and for each kind of name a target takes, the characters
of those that the target's names do not, first and after the first: Rust's and Python's
(XID_Start and XID_Continue), TypeScript's (ID_Start and ID_Continue) and Go's (the letters, Lu,
Ll, Lt, Lm and Lo, and the decimal digits, Nd). The lexers of ritsu's languages and the checks
read the letters and numbers of the file (`ident::is_letter`, `ident::is_letter_or_number`), not
those of `char`: the `char` of rustc 1.100 is of Unicode 18.0 and calls letters 12,833 characters
that rustc 1.100's names do not take, nor any target's yet. Read from the file, a source has the
same names whichever Rust built ritsu. chobo's E063 stops a build on a name with one of the
client's language, and its Rust clients read Rust's to write their names (chobo's DESIGN.md 14 and
18). kaname writes a name of a function's code as it is where the target takes it, and names it
`v` where not (kaname's DESIGN.md 36.3). rulec writes a name with no alias that has one with `_`
for it, in every language alike (rulec's DESIGN.md 15.311).

The files are not kept in this repository. To make the tables again, in `crates/ritsu-emit`:

```sh
curl -fsSL -o /tmp/DerivedCoreProperties.txt https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt
curl -fsSL -o /tmp/DerivedGeneralCategory.txt https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt
python3 tools/xid/make_table.py /tmp/DerivedCoreProperties.txt /tmp/DerivedGeneralCategory.txt
```

The script refuses files whose SHA-256 is not the one it was made from
(`24c7fed1195c482faaefd5c1e7eb821c5ee1fb6de07ecdbaa64b56a99da22c08` and
`d62e5bab70ca74f099343f71224fa051cb1fdd61a1ab45c0488c44cfc0b6102e`). For a new version of
Unicode, update `VERSION` and `FILES` in the script, chobo's DESIGN.md 14 and kaname's
DESIGN.md 36.3, and the paths above.

Two tests hold the tables to the tools that read the names. chobo's `tests/generated.rs`
(`e063_keeps_out_what_rustc_keeps_out`) puts every character a name of chobo's may begin with,
and every one it may hold after its first, through `cargo check` where E063 lets it be, and each
one E063 keeps out is an error that names it. kaname's `tests/idents.rs` puts every character a
name of kaname's may hold, first and after the first, through rustc, Python, tsc and Node, and Go,
and each tool takes the names its table lets in and none it keeps out, but for the characters
the tool's own Unicode does not have yet. When the identifiers of a new rustc read a newer
Unicode, rustc takes characters E063 keeps out, the two tests fail, and the tables are made again
from the newer files. A `char` that reads a newer Unicode alone, as rustc 1.100's does, changes
nothing: its new letters are in neither table of letters and numbers, and no lexer reads them
into a name.

The Unicode Character Database is under the Unicode License v3;
[THIRD_PARTY_NOTICES](../../THIRD_PARTY_NOTICES) gives it.
