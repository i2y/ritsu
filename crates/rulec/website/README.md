# rulec documentation site

Source for the user-facing rulec site, built with
[Zensical](https://zensical.org) (the Material for MkDocs successor by
the squidfunk team; latest release, unpinned).

## Layout

```
website/
├── zensical.toml     # English site (nav, palette, markdown extensions)
├── zensical.ja.toml  # Japanese site — docs-ja/ -> build/ja/
├── sync.sh           # pulls the repository's canonical documents in
├── build.sh          # both languages, in the one order that works
├── docs/             # English pages
└── docs-ja/          # Japanese pages
```

## One source of truth

Seven pages are **authored here**, in both languages: `index`, `install`,
`tour`, `checks`, `generate`, `compare`, `examples`. They are the tour,
written for a reader arriving at the site.

`examples.md` is **generated** by `tools/make_examples.py` from
`tests/corpus/*.rule`, so the sources on it cannot drift from the rules
the test suite actually runs; `tests/website.rs` holds it to them. Edit
the prose at the top of the script and re-run it. The output is
committed, so building the site needs no Python.

Five pages are **copied in by `sync.sh`** and are not committed — they
are the repository's own documents, and they are tested there:

| site page | comes from | what tests it |
|---|---|---|
| `agents.md` | `../AGENTS.md` | `tests/docs.rs` — links, commands, the worked example |
| `reference.md` | `../docs/reference.md` | `tests/codes.rs` — the reserved-word table against `src/kw.rs` |
| `formats.md` | `../docs/formats.md` | `tests/formats.rs` — every command's JSON keys |
| `generated-code.md` | `../docs/generated-code.md` | `tests/api.rs` — the signatures against the tool's output |
| `codes.md` | `../docs/codes.md` / `codes.ja.md` | `tests/codes.rs` — identical to `rulec explain --all` |

Editing one of those on the site would create a second copy that starts
drifting the same day. **Edit the repository document instead**, and
`sync.sh` brings it over.

`codes.md` is copied rather than regenerated, so building the site needs
no Rust toolchain: the checked-in `docs/codes.md` is already held to
`rulec explain --all --format markdown` by a test.

The four English references are copied into `docs-ja/` too, each with a
banner saying it is English and why. The alternative — a language
switcher that 404s, or a translation nobody keeps up — is worse than a
page that says what it is.

## Build and serve

```console
$ cd website
$ uv venv .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh                             # both languages, right order
$ python3 -m http.server 8001 -d build   # preview both, incl. /ja/
$ ./sync.sh && .venv/bin/zensical serve  # EN-only live preview on :8000
```

The English build cleans `build/`, so building only English silently
drops `build/ja` — always go through `./build.sh`.

Deployed by `.github/workflows/docs.yml` to GitHub Pages on every push
to main that touches `website/`, `AGENTS.md` or `docs/`.

## The front-page diagram

`docs/images/flow*.svg` — four files (two languages, two colour schemes) —
come from `tools/make_flow.py`, so the geometry cannot drift between them
and only the words change. It draws two kinds of shape and no others:
**cards** for the actors and **sheets** (a folded corner) for the things
that move between them, so that an arrow can only say "this actor produces
this thing, which that actor consumes". The script refuses to write a file
whose text would overflow a shape, which is what keeps a later wording
change from shipping unseen. Edit the text at the top of it and re-run:

```console
$ python3 tools/make_flow.py
```

The SVGs are committed: building the site must not need Python.

## Both configs carry

- `extra.alternate`, which renders the header language switcher;
- a unicode-preserving `toc.slugify`, so a Japanese heading anchors the
  way GitHub anchors it and a link written against the repository still
  lands.
