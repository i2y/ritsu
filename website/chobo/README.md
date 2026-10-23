# The chobo site

chobo's documentation, in English and Japanese, built with [Zensical](https://zensical.org) (the
successor of Material for MkDocs; built with 0.0.67, unpinned). It is one of the sites inside
ritsu's (`website/`, [its README](../README.md)), which builds it into `website/build/chobo` and
publishes it at <https://i2y.github.io/ritsu/chobo/>, and the Japanese site at
<https://i2y.github.io/ritsu/chobo/ja/>.

The tests, the examples and the documents this site copies in are chobo's, in `crates/chobo` of
ritsu's workspace; the paths below that name `examples/`, `tests/`, `src/` or `docs/` without a
directory are there.

## Layout

```
website/chobo/
├── zensical.toml       # the English site: docs/ -> build/
├── zensical.ja.toml    # the Japanese site: docs-ja/ -> build/ja/
├── build.sh            # both languages, in the one order that works
├── sync.sh             # copies chobo's reference in, and the pictures and the stylesheet into docs-ja/
├── docs/               # the English pages, the pictures and the stylesheet
│   └── doc/            # the examples as `chobo doc --format html` draws them
├── docs-ja/            # the Japanese pages
│   └── doc/            # the Japanese versions of the examples, drawn with `--lang ja`
└── tools/
    ├── booklexer.py    # colours the ```book blocks, here and on ritsu's pages
    └── make_chart.py   # cuts the chart on the home pages out of what `chobo doc` draws
```

The ```gate blocks of `together` are coloured by the lexer of ritsu's pages, `../tools/gatelexer.py`,
which both configurations name beside `booklexer`, and which `build.sh` puts on `PYTHONPATH` beside
`tools/`.

## One source of truth

The pages of the tour are **written here**: `index`, `install`, `tour`, `checks`, `scenarios`,
`databases`, `units`, `pages`, `together`, `examples`, `agents`, `assurance` and `design`. The
Japanese pages are written for a Japanese reader, not translated sentence by sentence, so the two
languages say the same things but not always in the same order, and their examples are the Japanese
versions (`examples/*/*.ja.book`). A change to one goes to the other by hand.

Four pages are **copied in by `sync.sh`** and are not committed: they are chobo's own documents, in
`crates/chobo/docs`, and they are tested there.

| site page | comes from | what tests it |
|---|---|---|
| `reference.md` | `docs/reference.md` | `tests/docs.rs`: its books, its links, and its table of keywords against `src/syntax.rs` |
| `formats.md` | `docs/formats.md` | `tests/docs.rs`: every command shown with its output prints it now |
| `targets.md` | `docs/targets.md` | `tests/docs.rs`: its commands and its links |
| `codes.md` | `docs/codes.md`, and `docs/codes.ja.md` for the Japanese site | `tests/docs.rs`: identical to `chobo explain --all --format markdown` |

Editing one of those on the site would make a second copy that starts drifting the same day:
**edit the document in `crates/chobo/docs` instead**, and `sync.sh` brings it over. A link of theirs
to a file of the crate (`../examples/…`) goes to the file on GitHub, as the skill's copies do. The
Japanese site has `codes.md` in Japanese; the other three are English, and each copy says so at the
top, rather than leaving a reader wondering whether a translation was lost.

## What holds the pages to the tool

The tests are chobo's, in `crates/chobo/tests`. `tests/docs.rs` (run by `cargo test`, and needing
nothing but the repository) reads every page written here, in both languages, and checks that:

- every diagnostic is word for word in a golden file of `tests/fixtures`, which the fixtures test
  holds to the checker, or, for what `ritsu check` says where a book meets the other languages, of
  `crates/ritsu/tests/golden`. Take an excerpt from a golden file; do not type one.
- every line of a ```` ```book ```` block is a line of a book of the examples, the tests or
  `chobo explain`. A line cut short with `…` must have its pieces, in that order, on one real line;
  a line without `…` is a whole line.
- every `chobo` command of a ```` ```console ```` block runs, from `crates/chobo`, and prints the
  lines shown with it, in order (a line that is only `…` stands for lines left out; a `--out` writes
  into a scratch directory).
- a ```` ```flow ````, ```` ```gate ````, ```` ```req ```` (or any other language's) block is the
  lines, in order, of one file of that language in the workspace; a ```` ```json ```` block of a
  scenario beside a book; a ```` ```text ```` block is in a golden file of chobo's or ritsu's; a
  block under `<!-- chobo build <book> --target <target> -->` is in a file that build writes; and a
  ```` ```mermaid ```` block is word for word on a page `chobo doc` wrote.
- every relative link leads where Zensical leads it (`use_directory_urls`): to a page of the site
  and a heading of it, to a file of the site (a page of an example, with the scenario, step and
  outcome a link opens), to a page of ritsu's site or of another language's, or to the playground
  in the page's language, on a project it lists in that language. A Japanese page's link to an
  image or to the stylesheet names the file of `docs/`, which `sync.sh` copies over.
- the English site and the Japanese one have the same pages, which both navs list in the same
  order, and each home page leads to every page written for its site.
- the counts the pages give (the scenarios, the codes) are the counts, and `tools/booklexer.py`'s
  `KEYWORDS` are `src/syntax.rs`'s, word for word and in their order (sekisho's `tests/docs.rs` holds
`../tools/gatelexer.py` to sekisho's lists the same way).

`tests/doc.rs` holds `docs/doc` and `docs-ja/doc` to what `chobo doc --format html` writes for the
examples now, and the charts of `docs/images` to what it draws; after a change to `doc`,
`CHOBO_BLESS=1 cargo test -p chobo --test doc` writes the pages anew, and `tools/make_chart.py` the
charts (below). ritsu's `crates/ritsu/tests/website.rs` holds what ritsu's site says of this one:
`build.sh` builds it, the home pages link to it, both navs and both home pages lead to the
playground in their language, the configurations name the URLs, and its blocks of a gate are
coloured.

## Build and preview

Zensical is installed once for ritsu's whole site, in `website/.venv`, and the whole site is built
and previewed from there:

```console
$ cd website
$ uv venv --python 3.13 .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh      # ritsu's pages, then each language's site, copied to build/chobo (and build/chobo/ja)
$ ./serve.sh      # http://localhost:8003/ritsu/chobo/ (./serve.sh <port> for another port)
```

`chobo/build.sh` builds this site alone, into `website/chobo/build`; `website/build.sh` runs it and
copies what it built into place. The English build cleans `build/`, so building only English
silently drops `build/ja`: always go through `build.sh`, which runs `sync.sh` first. It also clears
Zensical's page cache, which is keyed on the Markdown and would keep a page as it was after a change
to a lexer.

The pages link to `/ritsu/chobo/` absolutely (the language switch), where GitHub Pages serves them,
so `website/serve.sh` serves the site under `/ritsu/`; `python3 -m http.server -d build` would break
the header and the language switch.

## The chart on the home pages

`docs/images/chart*.svg` are four files, the refunds example in English and in Japanese, on the dark
and the light scheme. Nothing on them is drawn here: `tools/make_chart.py` runs `chobo doc
--format html` on the book, cuts out the chart of how things move as the page holds it, byte for
byte, and gives it the page's colours, its font and a background. Run it again after a change to
`doc` (with the chobo to run); the SVGs are committed, so building the site needs nothing but
Zensical.

```console
$ python3 tools/make_chart.py ../../target/debug/chobo      # in website/chobo
```

## The playground

The nav and the home page lead to ritsu's playground (`website/docs/playground.md`, published at
`/ritsu/playground/`), on the refunds; the Japanese site's, on the Japanese version. The playground
opens each of the four examples, in English and in Japanese, as a project of one book
(`chobo/inventory`, `chobo/inventory.ja` and so on), in the group `chobo`. ritsu's
`crates/ritsu/tests/playground.rs` holds the projects to the examples, and ritsu's site's README says
how to write them anew.

## Publishing

ritsu's `.github/workflows/docs.yml` builds the whole site with `website/build.sh` and deploys
`website/build/` to GitHub Pages, where this site is `/ritsu/chobo/`. It runs on a push to main that
changes what the site is built from (`website/`, `crates/chobo/docs/**` among the documents copied
in), and by hand from the Actions tab.
