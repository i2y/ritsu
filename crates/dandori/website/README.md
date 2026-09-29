# The dandori site

The documentation at <https://i2y.github.io/dandori/>, in English and Japanese, built with
[Zensical](https://zensical.org) (the successor of Material for MkDocs; built with 0.0.65, unpinned).

## Layout

```
website/
├── zensical.toml       # the English site: docs/ -> build/
├── zensical.ja.toml    # the Japanese site: docs-ja/ -> build/ja/
├── build.sh            # both languages, in the one order that works
├── sync.sh             # copies the pictures and the stylesheet into docs-ja/
├── serve.sh            # previews build/ under /dandori/, with caching off
├── docs/               # the English pages, the pictures and the stylesheet
│   └── doc/            # the examples as `dandori doc --format html` draws them
├── docs-ja/            # the Japanese pages
│   └── doc/            # the same, drawn with `--lang ja`
└── tools/
    ├── flowlexer.py    # colours the ```flow blocks
    └── make_overview.py  # draws the overview on the home page
```

The Japanese pages are written for a Japanese reader, not translated sentence by sentence, so the
two languages say the same things but not always in the same order. A change to one goes to the
other by hand.

## What holds the pages to the tool

`tests/docs.rs` (run by `cargo test`, and needing nothing but the repository) checks that:

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

`tests/doc.rs` (which needs rulec, and Chrome for two of its tests) holds `docs/doc` and
`docs-ja/doc` to what `dandori doc --format html` writes for the examples now; after a change to
`doc`, `DANDORI_BLESS=1 cargo test --test doc` writes them anew.

The agent skill (`skills/dandori`) carries copies of the English pages. After changing one, run
`skills/sync.sh`; `tests/skill.rs` fails until the copies match.

## Build and preview

```console
$ cd website
$ uv venv --python 3.13 .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh      # build/ (English) and build/ja (Japanese)
$ ./serve.sh      # http://localhost:8002/dandori/ (./serve.sh <port> for another port)
```

The English build cleans `build/`, so building only English silently drops `build/ja`: always go
through `./build.sh`. It also clears Zensical's page cache, which is keyed on the Markdown and
would keep a page as it was after a change to `tools/flowlexer.py`.

`serve.sh` serves the site under `/dandori/`, where GitHub Pages will, because the pages link to
that path; `python3 -m http.server -d build` would break the header and the language switch.

## The overview

`docs/images/overview*.svg` are four files, English and Japanese on the dark and the light
scheme, drawn by `tools/make_overview.py` from one layout, so only the words differ. Edit the words
at the top of the script and run it again; the SVGs are committed, so building the site needs
nothing but Zensical.

```console
$ .venv/bin/python tools/make_overview.py
```

## Publishing

`.github/workflows/docs.yml` builds both languages with `build.sh` and deploys `build/` to GitHub
Pages, at <https://i2y.github.io/dandori/>, on every push to main that touches `website/` or the
workflow, and by hand from the Actions tab. Pages takes its source from "GitHub Actions" in the
repository's settings.
