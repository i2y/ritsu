# The Shift_JIS table

`src/sjis_table.rs` is generated from the WHATWG Encoding Standard's index jis0208, the
table browsers read Shift_JIS pages with. koyomi reads the Cabinet Office's holiday CSV,
which is Shift_JIS, with it (DESIGN.md 1.5).

The index is not kept in this repository. To make the table again:

```sh
curl -fsSL -o /tmp/index-jis0208.txt https://encoding.spec.whatwg.org/index-jis0208.txt
python3 tools/sjis/make_table.py /tmp/index-jis0208.txt
```

The script refuses an index whose SHA-256 is not
`341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468`
(Identifier `cbaa91f3deb7d0841faf5c33041fc15a285da0e87e64ab802c4bf04b7c4da861`,
Date 2024-09-18). If the index was updated, read what changed, then update the digest in the
script and the identifier and date in DESIGN.md 1.5.

`tests/sources.rs` compares the decoder with Python's `cp932` on every pointer of the table
and on the whole CSV, when `python3` is available.

The index is part of the Encoding Standard, licensed under CC BY 4.0, and a portion of it
incorporated into source code, as `src/sjis_table.rs` is, is under the BSD 3-Clause License
instead; [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md) has both.
