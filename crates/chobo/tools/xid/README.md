# The table of the characters of Rust's names

`src/client/xid.rs` is generated from two files of the Unicode Character Database 17.0.0, the
version of the identifiers of rustc 1.94 to 1.97. It holds the characters a name of chobo's may
hold and a Rust name may not, where the name has them: E063 stops a build for a Rust target on a
name with one of them, and the Rust clients read it to write their names (DESIGN.md 14).

The files are not kept in this repository. To make the table again:

```sh
curl -fsSL -o /tmp/DerivedCoreProperties.txt https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt
curl -fsSL -o /tmp/DerivedGeneralCategory.txt https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt
python3 tools/xid/make_table.py /tmp/DerivedCoreProperties.txt /tmp/DerivedGeneralCategory.txt
```

The script refuses files whose SHA-256 is not the one it was made from
(`24c7fed1195c482faaefd5c1e7eb821c5ee1fb6de07ecdbaa64b56a99da22c08` and
`d62e5bab70ca74f099343f71224fa051cb1fdd61a1ab45c0488c44cfc0b6102e`). For a new version of
Unicode, update `VERSION` and `FILES` in the script and DESIGN.md 14, and the paths above.

`tests/generated.rs` holds the table to the rustc that runs the tests
(`e063_keeps_out_what_rustc_keeps_out`): every character a name of chobo's may begin with, and
every one it may hold after its first, goes through `cargo check` where E063 lets it be, and
each one E063 keeps out is an error that names it. When a new rustc and its `char` read a newer
Unicode, a character on which E063 and rustc part fails that test, and the table is made again
from the newer files.

The Unicode Character Database is under the Unicode License v3;
[THIRD_PARTY_NOTICES](../../THIRD_PARTY_NOTICES) gives it.
