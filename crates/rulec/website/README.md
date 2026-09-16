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

Eight pages are **authored here**, in both languages: `index`, `install`,
`tour`, `checks`, `generate`, `compare`, `examples`, `fit`. They are the tour,
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

## The front-page diagrams

Two pictures, eight files (two languages times two colour schemes each),
each drawn by one script from one layout so that the geometry cannot
drift between the files and only the words change:

| files | script | what it shows |
|---|---|---|
| `docs/images/overview*.svg` | `tools/make_overview.py` | what the tool does to a table: a row is a box, the boxes must tile the input space, and a gap comes back as the input that falls through it |
| `docs/images/flow*.svg` | `tools/make_flow.py` | who makes what and who takes it: the agent, rulec, a person |

Both draw with the vocabulary in `tools/diagram.py` — two kinds of shape
and no others, **cards** for the actors and **sheets** (a folded corner)
for the things that move between them, so that an arrow can only say
"this actor produces this thing, which that actor consumes" — and the
same three arrow colours. Either script refuses to write a file whose
text would overflow a shape, which is what keeps a later wording change
from shipping unseen. Edit the text at the top of a script and re-run it:

```console
$ python3 tools/make_overview.py
$ python3 tools/make_flow.py
```

The SVGs are committed: building the site must not need Python.

Nothing on the overview's sheets is made up: the table is
`tools/overview.rule` / `tools/overview-ja.rule`, the witness is what
`rulec check` says about that rule without its last row, and the code is
what `rulec gen` writes for it, line for line. `--verify` holds all three
to the tool, so run it after changing the table:

```console
$ python3 tools/make_overview.py --verify ../target/release/rulec
```

## Both configs carry

- `extra.alternate`, which renders the header language switcher;
- a unicode-preserving `toc.slugify`, so a Japanese heading anchors the
  way GitHub anchors it and a link written against the repository still
  lands.

## The playground

`docs/playground.md` (and its Japanese twin) is the checker itself, compiled to
wasm32 and running in the page: `check`, everything `gen` writes, and the
approver's page, with nothing sent anywhere. Three files sit beside it in
`docs/playground/`, and `sync.sh` copies them into `docs-ja/playground/` the way
it copies the images:

| file | what it is |
|---|---|
| `playground.js` | the page's side of the boundary — allocate, write, call, read the length out of the header. It finds the other two from its own URL |
| `playground.css` | borrows the theme's variables, so the palette and the dark-mode switch need no second set of colours |
| `rulec.wasm` | **a build product, committed**, so that building the site needs no Rust toolchain — the same bargain as the SVGs and Python |

Re-build it after anything that changes what `check`, `gen` or `doc` answer:

```console
$ website/tools/make_wasm.sh
```

`tests/wasm.rs` drives the committed file through node and holds its answers to
the binary's, byte for byte, in both languages — and holds `rulec_version` to the
crate's version. A stale `rulec.wasm` is a failing test, not a page that quietly
answers an old way.

## The screenshots

`docs/images/try-ja.png` and `try-en.png` are the page `rulec doc --format html` renders,
opened on one of the rule's own examples and photographed by headless Chrome:

```console
$ website/tools/shots.sh            # needs Google Chrome; uses ../target/debug/rulec
```

Re-run it after anything that changes the page. They are pictures of real output, not
mock-ups.
