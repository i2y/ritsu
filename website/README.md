# ritsu's site

ritsu's pages and the site of each language, in English and Japanese, built with
[Zensical](https://zensical.org) (the successor of Material for MkDocs; built with 0.0.67,
unpinned) into one tree, `build/`, which is published at <https://i2y.github.io/ritsu/>.

## Layout

```
website/
├── zensical.toml       # ritsu's English pages: docs/ -> build/
├── zensical.ja.toml    # ritsu's Japanese pages: docs-ja/ -> build/ja/
├── build.sh            # all of it, in the one order that works
├── sync.sh             # copies the playground into docs-ja/
├── serve.sh            # previews build/ under /ritsu/, with caching off
├── docs/               # the English pages: index.md, playground.md, sekisho.md
│   ├── ns/             # what a namespace IRI opens: yuen.md (see below)
│   └── playground/     # the playground: ritsu.wasm, projects.json, playground.js and .css
├── docs-ja/            # the Japanese pages (and ns/yuen.md)
├── playground/         # the shop the playground opens; projects.json is written from it, rulec's corpus and dandori's examples
├── tools/
│   └── make_wasm.sh    # builds docs/playground/ritsu.wasm
├── rulec/              # rulec's site: its own zensical.toml, build.sh and pages (its README)
└── dandori/            # dandori's site: its own zensical.toml, build.sh and pages (its README)
```

A language's site keeps its own configuration in its own directory, as it had in the language's
own repository, and its own `build.sh` builds it into `<language>/build`. `build.sh` here builds
ritsu's pages, then runs each language's `build.sh` (the list `sites` in it) and copies what it
built to `build/<language>`, published at `/ritsu/<language>/` and `/ritsu/<language>/ja/`. The two
sites are rulec's (`rulec/`, from rulec's repository) and dandori's (`dandori/`, from dandori's).
Each had a playground of its own; ritsu's took them over, with every example they opened and the
links they gave, and their `playground.md` is kept only to send a link to it on (the nav of each
site links to ritsu's page itself).

A language with no site of its own can have a page among ritsu's. sekisho's, `sekisho.md`, follows
its example from the answer of a rule a gate reads to the Cedar and the code generated from it,
with the dates, the contract, the workflow and the map around it; the nav lists it after the
playground, and the home page's table of the languages links to it. The table links the languages
with neither a site nor a page to their READMEs.

The Japanese pages are written for a Japanese reader, not translated sentence by sentence, so the
two languages say the same things but not always in the same order. A change to one goes to the
other by hand.

## What a namespace IRI opens

yuen writes the words PROV has none for in the namespace `https://i2y.github.io/ritsu/ns/yuen#`,
and `docs/ns/yuen.md` (`docs-ja/ns/yuen.md`, published at `/ritsu/ns/yuen/` and `/ritsu/ja/ns/yuen/`)
is the page that address opens. A word's IRI has no `/` before the `#`, so
`https://i2y.github.io/ritsu/ns/yuen#Requirement` is sent on to `…/ns/yuen/#Requirement`, and the
browser keeps the `#Requirement`. An id is matched with its case, and Zensical's own ids are lower
case, so every word has a heading written `### inForce { #inForce }`. The page is not in `nav`: a
person gets to it from an IRI, or from yuen's reference.

## The playground

`docs/playground.md` (and `docs-ja/playground.md`) runs ritsu in the page. Its module and the projects
it opens are committed, so building the site needs no Rust toolchain, and both go stale. At the root
of the repository:

```console
$ website/tools/make_wasm.sh    # after a change to what a language's check, gen or doc answers
$ RITSU_BLESS=1 cargo test -p ritsu --test playground the_projects_are_what_the_page_opens
                                # after a change to website/playground/, rulec's corpus or dandori's examples
```

`crates/ritsu/tests/playground.rs` fails until they are made anew.

## Build and preview

```console
$ cd website
$ uv venv --python 3.13 .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh      # build/ and build/ja, then build/rulec, build/rulec/ja, build/dandori, build/dandori/ja
$ ./serve.sh      # http://localhost:8003/ritsu/ (./serve.sh <port> for another port)
```

The English build of ritsu's pages cleans `build/`, so a language's site copied there before it
would be lost: always go through `./build.sh`. Every site's pages link to `/ritsu/` absolutely
(the language switch), where GitHub Pages will serve them, so `serve.sh` serves `build/` under that
path; `python3 -m http.server -d build` would break the header and the language switch.

## What holds the pages to the tool

`crates/ritsu/tests/website.rs` holds ritsu's pages: every relative link and anchor leads to a page,
a heading or a language's site that `build.sh` builds, and a link to a project of the playground
(`playground/#project=shop&file=gates/orders.gate`) to one it lists in the page's language. A link
that names no `.md` is read, as Zensical reads it, from the directory of the page's file, and is
written with one more `../` in a page published a directory deeper (`sekisho.md` at `/sekisho/`),
so `docs/sekisho.md` links to `rulec/`, not `../rulec/`, which would climb out of `/ritsu/`. Every
link into this repository names a file that is there; `docs/` and `docs-ja/` hold the same pages,
both navs list them in the same order (all but the page of yuen's namespace), and each home page
links to every other page of its nav; the code on the index is the lines of the files of the
languages, and `ritsu check` prints what it shows; the pages of sekisho show its example as the
tools have it: the lines of its files, what each command prints (a `gen` writes into a scratch
directory in place of its `--out`), the Cedar and the code `ritsu gen` writes, the table
`sekisho doc` draws and its count of combinations, and what `sekisho check` says of the example
with the edit a `diff` block shows, the two pages showing as many blocks of each kind and as many
tables (sekisho's READMEs link to the pages); the pages of yuen's
namespace have every word yuen's exporter writes (its list `TERMS`, which yuen's own tests hold to
what it writes) and no other, and their example is what the command prints; the two
configurations name ritsu's URLs; `.github/workflows/docs.yml` runs on a push to main that changes
what the site is built from, and by hand; and the page each of the two sites had for its
playground sends a reader on to the playground, where the site's nav and home page lead too. When
Zensical is in `.venv`, it runs `build.sh` into a copy of `website/` and looks at the tree it
builds, down to an element for every word whose id is the word, and to where the link of each page
that sends a reader on leads. `crates/ritsu/tests/playground.rs` holds the playground, as its page
says, and follows in Chrome the links those pages gave, sent on to it; each language's own tests
hold its site (rulec's and dandori's READMEs say how).

## Publishing

`.github/workflows/docs.yml` builds the whole site with `build.sh` and deploys `build/` to GitHub
Pages, on a push to main that changes what the site is built from (`website/`, the documents
rulec's `sync.sh` copies in, and the workflow itself), and by hand from the Actions tab. Pages
takes its source from "GitHub Actions" in the repository's settings. ritsu's site took over from
the sites rulec's and dandori's own repositories published, at <https://i2y.github.io/rulec/> and
<https://i2y.github.io/dandori/>.
