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
├── sync.sh             # copies the playground's files into docs-ja/
├── serve.sh            # previews build/ under /ritsu/, with caching off
├── docs/               # the English pages: index.md, playground.md, sekisho.md, node.md
│   ├── kaname/         # kaname's pages: index.md, language.md, examples.md, checks.md, together.md
│   ├── ns/             # what a namespace IRI opens: yuen.md (see below)
│   ├── play/           # index.html: the playground alone, filling the window (published as it is)
│   └── playground/     # the playground: playground.js and .css, worker.js, libraries.js, ritsu.wasm,
│                       #   projects.json, words.json
├── docs-ja/            # the Japanese pages (kaname/, ns/yuen.md and play/index.html too)
├── playground/         # the shop the playground opens; projects.json is written from it, rulec's corpus and dandori's examples
├── tools/
│   ├── knlexer.py      # colours the ```kn blocks (see below)
│   ├── make_wasm.sh    # builds docs/playground/ritsu.wasm
│   ├── make_libraries.sh  # bundles docs/playground/libraries.js from libraries/
│   └── libraries/      # what libraries.js holds: package.json, package-lock.json, entry.js, bundle.mjs
├── rulec/              # rulec's site: its own zensical.toml, build.sh and pages (its README)
├── dandori/            # dandori's site: its own zensical.toml, build.sh and pages (its README)
└── chobo/              # chobo's site: its own zensical.toml, build.sh and pages (its README)
```

A language's site keeps its own configuration in its own directory, as it had in the language's
own repository, and its own `build.sh` builds it into `<language>/build`. `build.sh` here builds
ritsu's pages, then runs each language's `build.sh` (the list `sites` in it) and copies what it
built to `build/<language>`, published at `/ritsu/<language>/` and `/ritsu/<language>/ja/`. The
sites are rulec's (`rulec/`, from rulec's repository), dandori's (`dandori/`, from dandori's) and
chobo's (`chobo/`, written here). rulec's and dandori's each had a playground of its own; ritsu's
took them over, with every example they opened and the links they gave, and their `playground.md`
is kept only to send a link to it on. The nav of each site links to ritsu's page itself, on a
project of the site's language, as chobo's does on its examples.

A language with no site of its own can have pages among ritsu's. sekisho's, `sekisho.md`, follows
its example from the answer of a rule a gate reads to the Cedar and the code generated from it,
with the dates, the contract, the workflow and the map around it; the nav lists it after the
playground, and the home page's table of the languages links to it. kaname's are five, in
`kaname/`: what it is, how a module is written, the four functional cores of its examples with
their hosts in TypeScript, Python and Go (in tabs, `pymdownx.tabbed`), what the check guarantees,
and the calls of rules, dates and books; the nav lists them after sekisho's, and the home page
links to each. The table links the languages with neither a site nor a page to their READMEs.

The Japanese pages are written for a Japanese reader, not translated sentence by sentence, so the
two languages say the same things but not always in the same order. A change to one goes to the
other by hand.

## The colours of the code

Pygments colours the code of the languages when the site is built, as it colours every other block:
the theme's light and dark palettes are written against Pygments' classes, so nothing runs in the
browser. `tools/knlexer.py` reads the `kn` fences, and ritsu's pages use the lexers of the languages'
sites as they are, not copies of them: `rulec/tools/rulelexer.py` for `rule`,
`dandori/tools/flowlexer.py` for `flow` and `chobo/tools/booklexer.py` for `book`. A lexer is imported
only by being named as a Markdown extension, so both configurations name the four, and `build.sh`
puts the four directories on `PYTHONPATH`; it also clears Zensical's page cache, which is keyed on the
Markdown and would keep a page as it was after a change to a lexer. A fence of a language with no
lexer (`gate`, `cedar`) is built as plain text.

Each lexer keeps the words of its language a second time, in Python, and the language's tests hold
them to the language's own lists: kaname's `tests/docs.rs` holds `knlexer.py` to `src/kw.rs`, and
the characters it reads a number with to `src/lex.rs`, as rulec's `tests/website.rs` and the
`tests/docs.rs` of dandori and chobo hold theirs. `knlexer.py` colours a word by where kaname's parser
reads it: `count` is a bound after a type and a builtin in a call, `total` an effect after a
function's answer and a name anywhere else, and a word before `:` a name, whatever its spelling.

## What a namespace IRI opens

yuen writes the words PROV has none for in the namespace `https://i2y.github.io/ritsu/ns/yuen#`,
and `docs/ns/yuen.md` (`docs-ja/ns/yuen.md`, published at `/ritsu/ns/yuen/` and `/ritsu/ja/ns/yuen/`)
is the page that address opens. A word's IRI has no `/` before the `#`, so
`https://i2y.github.io/ritsu/ns/yuen#Requirement` is sent on to `…/ns/yuen/#Requirement`, and the
browser keeps the `#Requirement`. An id is matched with its case, and Zensical's own ids are lower
case, so every word has a heading written `### inForce { #inForce }`. The page is not in `nav`: a
person gets to it from an IRI, or from yuen's reference.

## The playground

One component runs ritsu in the browser, on two pages: `docs/playground.md` (and
`docs-ja/playground.md`), which explains it and stacks the code above what ritsu makes of it, and
`docs/play/index.html` (and `docs-ja/play/index.html`), where it fills the window, the code on the
left and the results on the right. Zensical publishes an HTML file of `docs/` as it is, at
`/ritsu/play/`. Both pages load the same files from `docs/playground/` (`sync.sh` copies them for the
Japanese pages): the component, the worker that runs `ritsu.wasm`, and `libraries.js`, the editor
(CodeMirror 6) and the Markdown (marked) bundled into one file, so that the page loads nothing from
anywhere else. The module, the libraries, the projects and the words the editor colours are
committed, so building the site needs neither Rust nor Node, and each goes stale. At the root of the
repository:

```console
$ website/tools/make_wasm.sh        # after a change to what a language's check, gen or doc answers
$ website/tools/make_libraries.sh   # after a change to tools/libraries (npm installs the lock as it is)
$ RITSU_BLESS=1 cargo test -p ritsu --test playground the_projects_are_what_the_page_opens
                                    # after a change to website/playground/, rulec's corpus or dandori's examples
$ RITSU_BLESS=1 cargo test -p ritsu --test playground the_editor_colours_the_words_each_language_reserves
                                    # after a language gains or loses a keyword
```

`crates/ritsu/tests/playground.rs` fails until they are made anew. The packages of
`tools/libraries` are pinned to versions out for two weeks or more when they were locked
(`npm install --before=<date>`); the audit's osv-scanner reads their lock with the repository's
others. What npm installs there (`node_modules`) is not committed.

## Build and preview

```console
$ cd website
$ uv venv --python 3.13 .venv && uv pip install --python .venv/bin/python zensical
$ ./build.sh      # build/ and build/ja, then build/<site> and build/<site>/ja for rulec, dandori and chobo
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
tables (sekisho's READMEs link to the pages); the pages of kaname show its examples and the
materials of its tests in the same way, the code of the hosts being the lines of the hosts of its
examples and a `text` block after them what they print (`host/output.txt`), the page of the
examples showing all twelve hosts, and a page in English and the same page in Japanese showing as
many blocks of each kind and as many tables (kaname's READMEs and the READMEs at the root link to
them); the pages of yuen's
namespace have every word yuen's exporter writes (its list `TERMS`, which yuen's own tests hold to
what it writes) and no other, and their example is what the command prints; the two
configurations name ritsu's URLs; `.github/workflows/docs.yml` runs on a push to main that changes
what the site is built from, and by hand; the nav and the home page of each language's site lead to
the playground in the site's language, on a project the playground lists in it; the page each of
rulec's and dandori's sites had for its playground sends a reader on to the same place; and
the page of the playground alone leads to the page that explains it and to the playground's files,
and nowhere else. When Zensical is in `.venv`, it runs `build.sh` into a copy of `website/` and
looks at the tree it builds, down to an element for every word whose id is the word, to the four
sets of three tabs on the page of kaname's examples, to where the link of each page that sends a
reader on leads, to the pages of the playground alone, as they are, and to the colours of the code:
every `kn`, `rule`, `flow` and `book` fence of ritsu's pages is built by its lexer, with Pygments'
classes on its words and nothing the lexer could not read, and the pages of kaname show a keyword, a
type, a number with its unit, a date, a comment, a string and an operator, each in its class.
`crates/ritsu/tests/playground.rs` holds the playground, as its page says: it drives Chrome
through both pages in both languages (an edit, the answer, the page kept while a file does not
pass, a link from one page to the other) and follows the links the pages before gave, sent on to
it; each language's own tests hold its site (the READMEs of rulec's, dandori's and chobo's sites say
how).

## Publishing

`.github/workflows/docs.yml` builds the whole site with `build.sh` and deploys `build/` to GitHub
Pages, on a push to main that changes what the site is built from (`website/`, the documents
rulec's and chobo's `sync.sh` copy in, and the workflow itself), and by hand from the Actions tab. Pages
takes its source from "GitHub Actions" in the repository's settings. ritsu's site took over from
the sites rulec's and dandori's own repositories published, at <https://i2y.github.io/rulec/> and
<https://i2y.github.io/dandori/>.
