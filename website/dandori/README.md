# The dandori site

dandori's documentation, in English and Japanese, built with [Zensical](https://zensical.org) (the
successor of Material for MkDocs; built with 0.0.67, unpinned). It is one of the sites inside
ritsu's (`website/`, [its README](../README.md)), which builds it into `website/build/dandori` and
publishes it at <https://i2y.github.io/ritsu/dandori/>. It took over from the site dandori's own
repository published, at <https://i2y.github.io/dandori/>.

## Layout

```
website/dandori/
├── zensical.toml       # the English site: docs/ -> build/
├── zensical.ja.toml    # the Japanese site: docs-ja/ -> build/ja/
├── build.sh            # both languages, in the one order that works
├── sync.sh             # copies the pictures and the stylesheet into docs-ja/
├── docs/               # the English pages, the pictures and the stylesheet
│   └── doc/            # the examples as `dandori doc --format html` draws them
├── docs-ja/            # the Japanese pages
│   └── doc/            # the Japanese versions of the examples, drawn with `--lang ja`
└── tools/
    ├── flowlexer.py    # colours the ```flow blocks
    └── make_overview.py  # draws the overview on the home page
```

The Japanese pages are written for a Japanese reader, not translated sentence by sentence, so the
two languages say the same things but not always in the same order. A change to one goes to the
other by hand.

## What holds the pages to the tool

The tests are dandori's, in `crates/dandori/tests`, and the paths they name are dandori's crate's
(`examples/`, `tests/`, `src/`) or this directory's. `tests/docs.rs` (run by `cargo test`, and needing
nothing but the repository) checks that:

- every diagnostic shown on a page, and in the README, is word for word in a golden file of
  `tests/fixtures`, which the fixtures test holds to what the checker prints. Take an excerpt
  from a golden file; do not type one.
- every line of a ```` ```flow ```` block is a line of a `.flow` under `examples/` or `tests/`. A
  line cut short with `…` must have its pieces, in that order, on one real line.
- `reference/codes.md` in both languages lists every code the checker has and no other, and the
  pages that say how many there are say the right number.
- every ```` ```mermaid ```` block is word for word in a golden file of `tests/doc`, which
  `tests/doc.rs` holds to what `dandori doc` writes. Take a chart from a golden file.
- `tools/flowlexer.py`'s `KEYWORDS` are `src/syntax.rs`'s, word for word.

`tests/doc.rs` (which needs Chrome for two of its tests) holds `docs/doc` and
`docs-ja/doc` to what `dandori doc --format html` writes for the examples now; after a change to
`doc`, `DANDORI_BLESS=1 cargo test -p dandori --test doc` writes them anew.

The agent skill (`crates/dandori/skills/dandori`) carries copies of the English pages. After
changing one, run `crates/dandori/skills/sync.sh`; `tests/skill.rs` fails until the copies match.

## Build and preview

Zensical is installed once for ritsu's whole site, in `website/.venv`, and the whole site is built
and previewed from there:

```console
$ cd website
$ uv venv --python 3.13 .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh      # ritsu's pages, then this site, copied to build/dandori (and build/dandori/ja)
$ ./serve.sh      # http://localhost:8003/ritsu/dandori/ (./serve.sh <port> for another port)
```

`dandori/build.sh` builds this site alone, into `website/dandori/build`; `website/build.sh` runs it
and copies what it built into place. The English build cleans `build/`, so building only English
silently drops `build/ja`: always go through `build.sh`. It also clears Zensical's page cache, which
is keyed on the Markdown and would keep a page as it was after a change to `tools/flowlexer.py`.

The pages link to `/ritsu/dandori/` absolutely (the language switch), where GitHub Pages will serve
them, so `website/serve.sh` serves the site under `/ritsu/`; `python3 -m http.server -d build` would
break the header and the language switch.

## The overview

`docs/images/overview*.svg` are four files, English and Japanese on the dark and the light
scheme, drawn by `tools/make_overview.py` from one layout, so only the words differ. Edit the words
at the top of the script and run it again; the SVGs are committed, so building the site needs
nothing but Zensical.

```console
$ ../.venv/bin/python tools/make_overview.py      # in website/dandori
```

## The playground

The playground this site had is ritsu's now (`website/docs/playground.md`, published at
`/ritsu/playground/`): it opens every flow of the examples with the files the flow reads, and a reader
can change the rules there too. The nav and the home page link to it. `playground.md` in each language
is kept only to send a link to it on: a link it gave (`#flow=examples/hotel/temporal/hotel.flow&view=build`)
opens the same flow there, and any other opens the first draft it opened on. ritsu's
`crates/ritsu/tests/playground.rs` holds the page, the examples in it and the sending on, and ritsu's
site's README says how to build the page's module and write its projects anew.

What `rulec doc` renders names the version of rulec, so a new rulec means drawing `docs/doc` and
`docs-ja/doc` anew (`DANDORI_BLESS=1 cargo test -p dandori --test doc`).

## Publishing

ritsu's `.github/workflows/docs.yml` builds the whole site with `website/build.sh` and deploys
`website/build/` to GitHub Pages, where this site is `/ritsu/dandori/`. It runs on a push to main
that changes what the site is built from, and by hand from the Actions tab (`workflow_dispatch`).
Pages takes its source from "GitHub Actions" in the repository's settings.
