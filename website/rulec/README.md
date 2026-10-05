# rulec documentation site

Source for the user-facing rulec site, built with
[Zensical](https://zensical.org) (the Material for MkDocs successor by
the squidfunk team; latest release, unpinned). It is one of the sites inside
ritsu's (`website/`, [its README](../README.md)), which builds it into
`website/build/rulec` and publishes it at <https://i2y.github.io/ritsu/rulec/>.
It took over from the site rulec's own repository published, at
<https://i2y.github.io/rulec/>.

The tests, the corpus and the documents this site copies in are rulec's, in
`crates/rulec` of ritsu's workspace; the paths below that name `tests/`, `src/`,
`AGENTS.md` or `docs/` without a directory are there.

## Layout

```
website/rulec/
├── zensical.toml     # English site (nav, palette, markdown extensions)
├── zensical.ja.toml  # Japanese site — docs-ja/ -> build/ja/
├── sync.sh           # pulls rulec's canonical documents in, from crates/rulec
├── build.sh          # both languages, in the one order that works
├── docs/             # English pages
├── docs-ja/          # Japanese pages
└── tools/            # the diagrams, the screenshots, the examples page
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
are rulec's own documents, in `crates/rulec`, and they are tested there:

| site page | comes from | what tests it |
|---|---|---|
| `agents.md` | `crates/rulec/AGENTS.md` | `tests/docs.rs` — links, commands, the worked example |
| `reference.md` | `crates/rulec/docs/reference.md` | `tests/codes.rs` — the reserved-word table against `src/kw.rs` |
| `formats.md` | `crates/rulec/docs/formats.md` | `tests/formats.rs` — every command's JSON keys |
| `generated-code.md` | `crates/rulec/docs/generated-code.md` | `tests/api.rs` — the signatures against the tool's output |
| `codes.md` | `crates/rulec/docs/codes.md` / `codes.ja.md` | `tests/codes.rs` — identical to `rulec explain --all` |

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

Zensical is installed once for ritsu's whole site, in `website/.venv`, and the
whole site is built and previewed from there:

```console
$ cd website
$ uv venv --python 3.13 .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh      # ritsu's pages, then this site, copied to build/rulec (and build/rulec/ja)
$ ./serve.sh      # http://localhost:8003/ritsu/rulec/ (./serve.sh <port> for another port)
```

`rulec/build.sh` builds this site alone, into `website/rulec/build`;
`website/build.sh` runs it and copies what it built into place. The English
build cleans `build/`, so building only English silently drops `build/ja` —
always go through `build.sh`. The pages link to `/ritsu/rulec/` absolutely (the
language switch), where GitHub Pages will serve them, so `website/serve.sh`
serves the site under `/ritsu/`.

Published by ritsu's `.github/workflows/docs.yml`, which builds the whole site
with `website/build.sh`, on a push to main that changes what the site is built
from, and by hand (ritsu's README for the site says more).

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
$ python3 tools/make_overview.py --verify ../../target/release/rulec
```

## Both configs carry

- `extra.alternate`, which renders the header language switcher;
- a unicode-preserving `toc.slugify`, so a Japanese heading anchors the
  way GitHub anchors it and a link written against the repository still
  lands.

## The playground

The playground this site had (§15.48 of rulec's DESIGN) is ritsu's now
(`website/docs/playground.md`, published at `/ritsu/playground/`): it opens the five rules this site's
page offered, in both languages, beside the examples of the other languages, and a rule of the
reader's own in an empty project. The nav and the home page link to it. `playground.md` in each
language is kept only to send a reader on to it, on the table with a row missing that the page
opened on (a link that names a project of ritsu's page goes on as it is).
ritsu's `crates/ritsu/tests/playground.rs` holds the page to the tool, the examples in it to the
corpus and to `tools/overview.rule`, and the sending on; ritsu's site's README says how to build the
page's module and write its projects anew.

## The social preview

`docs/images/social.png` is what GitHub, Slack and the rest show when a link
to the repository is posted — 1280×640, the size GitHub asks for. It is the
site's own hero, drawn as one HTML document and photographed by the same
headless Chrome as the screenshots below, so the ink, the mark and the ruling
behind them cannot drift from the page:

```console
$ website/rulec/tools/social.sh      # needs Google Chrome
```

**GitHub has no API for it.** The file is uploaded by hand, once, under
Settings → General → Social preview.

## The screenshots

`docs/images/try-ja.png` and `try-en.png` are the page `rulec doc --format html` renders,
opened on one of the rule's own examples and photographed by headless Chrome:

```console
$ website/rulec/tools/shots.sh      # needs Google Chrome; uses the workspace's debug rulec
$ website/rulec/tools/shots.sh ../../target/debug/rulec en    # only the English pictures
```

Each language shows its own rule: the Japanese pictures are of `tests/corpus/送料.rule`, the
English ones of `tests/corpus/parcel_rate.rule`, a tariff written in English (pounds, inches,
USD). Re-run it after anything that changes the page. They are
pictures of real output, not mock-ups.
