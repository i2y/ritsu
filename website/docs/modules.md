# Modules

A module is a directory with a `ritsu.mod`: a project that other projects require, from a directory
beside them, a git repository or an OCI registry, and whose files they read by `@<name>/…` paths.
The modules published for ritsu are on [the list of modules](modules/list/), where you can search
them, read and try what they hold, and see what changed between versions.

A project checks the files it requires with its own files, with each language's check and the checks
across the languages, so a proof does not stop at a module's edge. The examples on this page are the
three modules of ritsu's tests,
[crates/ritsu/tests/projects/modules](https://github.com/i2y/ritsu/tree/main/crates/ritsu/tests/projects/modules):
`tax` holds the VAT rates, `pricing` the shipping fees and the order totals, which apply `tax`'s
rate, and `shop` an order flow and a basket, which `pricing` prices.

## Writing a module

`ritsu mod init <name>` writes a `ritsu.mod` in the directory it runs in, and that directory is a
module from then on. Here is `pricing`'s:

<!-- crates/ritsu/tests/projects/modules/pricing/ritsu.mod -->
```text
# The prices of the Acme shop: shipping fees, and order totals with the VAT rate of tax.
module example.com/acme/pricing
version 1.1.0
description "Shipping fees and order totals of the Acme shop"
license "MIT OR Apache-2.0"

require tax path "../tax"
```

A line of `ritsu.mod` is one statement, and `#` starts a note that runs to the end of the line.
`module` comes first.

| Line | What it says |
|---|---|
| `module <name>` | The module's name. A module pushed to a registry is named by its repository there (`example.com/acme/pricing`, `ghcr.io/i2y/ritsu-holidays`), and a module shared by git by where it is (`example.com/acme/rates`, [below](#sharing-a-module-by-git)). A module only read by a path may have a short name (`shop`). |
| `version <version>` | The version, in SemVer's form, without a `v` and without build metadata (`1.4.0`, `2.0.0-rc.1`). It is the tag the module is pushed as, and, after a `v`, the tag of a module shared by git (`v1.4.0`). |
| `ritsu >=<version>` | The least ritsu that reads the module. An older one stops and says which version it needs. |
| `description`, `license`, `source` | What the registry, the list of modules and the libraries show. `license` is an SPDX expression. `source` is the https URL of the module's repository, with no user, password or query (E302), as it is published. |
| `require <name> path "<dir>"` | A dependency in a directory, written from the module's root. |
| `require <name> oci "<repository>" <version>` | A dependency from a registry, at that version or a later one. |
| `require <name> git "<URL>" <version> [dir "<dir>"]` | A dependency from the tags of a git repository, at that version or a later one; `dir` is the module's directory in the repository ([Sharing a module by git](#sharing-a-module-by-git)). |
| `exclude "<path>"` | A file or a directory that `ritsu mod push` leaves out. `ritsu mod init` writes `exclude "generated"`, where `ritsu gen` writes. |
| `signature "<name>" …` | The signature the modules of that name must have ([Signatures](#signatures)). |
| `library typescript\|python\|go "<name>"` | The name of the module's library in that language ([Libraries](#libraries)). |
| `target typescript\|python\|go\|rust\|cedar …` | What `ritsu gen` writes when it is given no `--target`, a library as well ([Libraries](#libraries)). Only `ritsu gen` reads it. A ritsu older than 0.41.0 cannot read it, so a module with it asks for `ritsu >=0.41.0`. |

The name of a dependency is the word after `require`: lower-case letters, digits and `_`, starting
with a letter. The module's files name a file of the dependency by that name, with `@`, and the
path from the dependency's root. `order_total.rule` of `pricing` applies the rate of `tax`:

```rule
rule order_total v1
…
apply vat = "@tax/rules/vat_rate.rule" meaning:8a62fb4ec05fd289
```

A `@` path is read from the module's root, whichever directory the file is in, and every language
reads it where it reads a path: a rule's `apply` and `import`, a workflow's `use rule`, a calendar's
`use calendar`, a gate's `use`, a `use` of kaname, a link of yuen, a context of sakai. A path given
on the command line inside a module is read the same way: `ritsu rulec doc @tax/rules/vat_rate.rule`
writes the page of the dependency's rule, and `ritsu mod where` says where the file is on the disk.

- The root of a module is where its `ritsu.mod` is. ritsu takes the nearest directory above the
  files it is given that holds a `.git` or a `ritsu.mod`, unless `--root` names another.
- A path stays in its module. One that leaves the module's root, or goes into a module below it
  without `@`, is an error of the language that reads it. So is a file or a directory at the root
  whose name starts with `@` (E307).
- A directory below with a `ritsu.mod` of its own is another module. A walk over the module does not
  go into it. Run from above it, `ritsu check` says how to check it by itself.
- In a project with no `ritsu.mod`, nothing changes: a path that starts with `@` is a path like any
  other.

## Requiring a module

`ritsu mod add <name> <source>` requires a module: from a directory, as `./…` or `../…` from where
it runs, from a registry, as `<repository>@<version>` or `<repository>@latest`, or from a git
repository, as `<URL>@<version>` or `<URL>@latest` (`@latest`, or no version, takes the latest
version that is not a pre-release). It writes the `require` line after the last one, and
`ritsu.lock` when the module needs one.

```console
$ ritsu mod add pricing ../pricing
$ ritsu mod add holidays ghcr.io/i2y/ritsu-holidays@1.1.0
$ ritsu mod add rates https://example.com/acme/rates@1.2.0
$ ritsu mod get holidays@latest
$ ritsu mod remove holidays
```

The name is the requiring project's to choose: it is what the project's files write after `@`.
[The list of modules](modules/list/), this page and the module's README all call the module of
holidays `holidays`.

`ritsu mod get <name>@<version>` changes the version of a dependency from a registry or a git
repository, and `ritsu mod remove <name>` removes its line. The three write `ritsu.mod` and
`ritsu.lock` together or not at all, and keep the other lines of `ritsu.mod` and their notes as they
are.

`ritsu.lock` is ritsu's to write. It has a line for every module the project reads, the dependencies
of its dependencies too: for a module from a registry, the version chosen and two digests, the OCI
manifest's, which the registry and cosign know, and the digest of the module's files; for a module
from git, the same, the commit its tag led to and the repository's URL. The version is
chosen by Minimal Version Selection: for each module, the highest of the versions the `require`
lines ask for, and one version of each module. So `ritsu.mod` decides the lock with no question to a
registry, and two machines with the same files read the same files of every module. A module that
requires nothing from a registry or a git repository needs no lock, and has none.

- Every command checks the lock against `ritsu.mod`, and stops at a lock that does not match it
  (E304). `ritsu mod lock` writes it again, and `ritsu mod lock --check` exits 1 when it would
  change: a step of CI.
- The files of a dependency from a registry or a git repository are checked against the lock's
  digest whenever they are read (E305).
- A `require … path` in the working tree wins over a `require … oci` or a `require … git` of the
  same module anywhere in the tree. Writing one is how a dependency is changed and checked from the
  module that uses it.

`ritsu mod list` lists the modules the project reads:

```console
$ cd crates/ritsu/tests/projects/modules/shop
$ ritsu mod list
module example.com/acme/shop 0.1.0
@pricing       example.com/acme/pricing 1.1.0  path ../pricing
@pricing/@tax  example.com/acme/tax 1.0.0      path ../tax
ritsu mod list: 2 dependencies (2 by a path, 0 from a registry); no lock is needed
```

`ritsu check` in a module reads every module the project reads, and checks their files as it checks
its own, the checks across the languages included:

```console
$ cd crates/ritsu/tests/projects/modules/shop
$ ritsu check .
ok checkout/basket.kn: 2 functions, total
orders/place_order.flow: ok
dependencies: 2 modules, 3 files (rulec 3): all pass
ritsu check: 2 files (kaname 1, dandori 1): all pass; borders between the languages: 0 checked, 0 undecided
```

- An error in a dependency's file fails the check. Its warnings are counted on the line of the
  dependencies, and `--deps-warnings` shows each.
- A `require` whose files no file of the module names is W301. A file that names a dependency of a
  dependency (`@pricing/@tax/…`) is W303: require that module in your own `ritsu.mod`.
- A dependency's `.geas` claims are not run: they are about programs of the dependency's author,
  which are not on your machine.
- A `contribution` to a dependency's rule adds to that rule in the project, and `ritsu check` notes
  each one that crosses a module's edge.

## Fetching what a module requires

Only `ritsu mod` reaches the network. `ritsu check`, `gen`, `run` and the languages' commands read a
dependency from a registry or a git repository where `ritsu mod fetch` has put it. When it is not
there, they stop and say to run `ritsu mod fetch`.

```console
$ ritsu mod fetch
$ ritsu mod vendor
```

- `ritsu mod fetch` fetches the modules of the lock that are not here yet into the store, a
  directory kept across runs: `RITSU_MODULES`, else `~/Library/Caches/ritsu/modules` on macOS and
  `$XDG_CACHE_HOME/ritsu/modules` (or `~/.cache/ritsu/modules`) elsewhere. It checks the digests,
  and the signatures `signature` lines ask for, and writes nothing unless every module passes.
  `--rev <revision>` fetches what the lock at that git revision pins, and `<name>@<version>` that
  version.
- `ritsu mod vendor` puts a copy of them in the module, in `.ritsu/modules/`, for a build that
  cannot reach the registry or the repository, or to keep them in the repository.
  `ritsu mod vendor --check` says whether the copy matches the lock. Each command reads the copy
  before the store.
- `ritsu mod clean` removes what the store holds, or with `--keep <lock>…` what those locks do not
  pin. Anything removed is fetched again by its digest.
- `ritsu mod where @<name>/<path>` says where a file of a dependency is on the disk, for an editor.

A registry is asked over HTTPS with curl (plain HTTP only on this machine). The first request goes
without credentials. When the registry asks for them, ritsu reads Docker's configuration file
(`$DOCKER_CONFIG/config.json`, else `~/.docker/config.json`), as `docker login` and the credential
helpers write it, and runs the helper it names. No output, log or file shows a credential or a
token.

The npm package reads the store and the module's copy as the native ritsu does. The commands that
reach a registry or run git or cosign (`add`, `get`, `fetch`, `versions`, `push`, `report` and
`site`, and `lock`, `remove` and `diff` when they have to fetch) say to use the native ritsu. When
the API's `dirs` opens only some directories, add the store to `dirs`, or put the copy in the module
with `ritsu mod vendor`.

## Sharing a module by git

A module shared among your own repositories needs no registry. Its author tags a commit
`v<version>` and pushes the tag, and a project requires the module by the repository's URL. A
module in a directory of a repository takes `--dir`, and its tags are `<dir>/v<version>`:

```console
$ ritsu mod add rates https://example.com/acme/rates@1.2.0
$ ritsu mod add tax git@example.com:acme/mono.git@1.0.0 --dir modules/tax
$ ritsu mod versions https://example.com/acme/rates
```

```text
require rates git "https://example.com/acme/rates" 1.2.0
require tax git "git@example.com:acme/mono.git" 1.0.0 dir "modules/tax"
```

- A URL is `https://…`, `ssh://…` or the scp form `<user>@<host>:<path>`. `git://`, `file://`,
  plain `http://` to another machine, `<transport>::…`, and a URL with a password or a query in it
  are E302.
- The module's name is where it is: the host, the repository's path without `.git`, and the `dir`,
  with no scheme, user or port (`example.com/acme/rates`, `example.com/acme/mono/modules/tax`). The
  `ritsu.mod` at the tag says that `module` and the tag's version.
- Names are in lower case: write the URL in lower case. Many hosts do not tell the cases of a
  repository's path apart; on one that does, a repository with an upper-case letter in its path
  cannot be required.
- A tag without `v`, with `+`, or not in SemVer's form is not a version. A pre-release is one, but
  `@latest` does not take it.
- The lock pins the commit the tag led to, with the URL. `ritsu mod fetch` fetches that commit by
  its name, with no checkout: the commit and its trees, then the files of the module at once, read
  as git holds them, with no filter, no change of the ends of the lines and no hook. A symbolic
  link, a submodule or a Git LFS pointer in the module stops it.
- A moved tag is seen as a registry's is: `ritsu mod fetch` keeps to the lock's commit,
  `ritsu mod get` takes the commit the tag leads to now, and `ritsu mod report` puts the move first.
  When the repository no longer gives the lock's commit, `ritsu mod fetch` stops and says that the
  tag moved.
- ritsu runs git, 2.30 or later, with your settings: ssh keys, credential helpers and
  `url.<base>.insteadOf` work as they do for `git fetch`. ritsu does not let git ask for a password
  on the terminal: with no credential, it stops at once.

`japan_liquor_tax` on [the list of modules](modules/list/) is shared this way: a public repository of
its own with the tags `v1.0.0`, `v1.1.0` and `v1.2.0`, one for each step of a reform of the liquor
tax. To see what a step moved, require the module, and compare two versions in that project:

```console
$ ritsu mod versions https://github.com/i2y/ritsu-japan-liquor-tax
$ ritsu mod add japan_liquor_tax https://github.com/i2y/ritsu-japan-liquor-tax@1.2.0
$ ritsu mod diff github.com/i2y/ritsu-japan-liquor-tax@1.1.0 github.com/i2y/ritsu-japan-liquor-tax@1.2.0
```

A module shared by git has no signature to check: the lock's commit and digests hold it, and a
`signature` line that covers it is E309. A module that requires one from git cannot be pushed to a
registry. Since the name of a module from git is where it is, one module is shared one of the two
ways. To move a module from git to a registry, change its `module` line to the registry's repository
and push a new version. The projects that use it change their `require … git` line to
`require … oci` and run `ritsu mod lock`; their files name the module by the name they gave it, and
do not change.

In GitHub Actions, the token `actions/checkout` keeps is for the repository it checks out, and reads
no other. ritsu runs git in a repository of its own, which does not use it. A job that requires a
module from another private repository gives git a token that reads it, kept as a secret (here
`MODULES_TOKEN`), through git's global configuration. A credential helper that reads the token from
the environment keeps it out of the files, with the user name the host takes with a token:

```sh
git config --global credential.https://example.com.username ci
git config --global credential.https://example.com.helper '!f() { test "$1" = get && echo "password=$MODULES_TOKEN"; }; f'
```

Run these in a step before ritsu's, and give the step that runs `ritsu mod fetch` the secret in its
`env` as `MODULES_TOKEN`.

## Pushing a module to a registry

`ritsu mod push` pushes the module to the repository its name names, as the tag of its version:

```console
$ cd modules/holidays
$ ritsu mod push --dry-run
built ghcr.io/i2y/ritsu-holidays 1.1.0, and pushed nothing (`--dry-run`)
…
```

1. It takes a module with a name of a registry, a `version` and a `ritsu >=` line, no
   `require … path` or `require … git`, and a lock that matches.
2. It gathers the module's files, all but what `exclude` names, the names that start with `.` and the
   modules below, and refuses the module when one of them holds a key (W901, said by file and line,
   never the key).
3. It checks a copy of those files with `ritsu check`, and refuses the module when there is an
   error. The module's own `.geas` claims are left to `ritsu check` in the working tree, where the
   programs they run are.
4. It builds the OCI artifact. The same files always give the same bytes and the same digest, so
   anyone can build the digest from the source and compare. `--dry-run` stops here and prints it.
5. A tag that holds the same files is pushed already, and one that holds other files refuses the
   push. A version is never moved: raise it.
6. It pushes what the registry does not have, and prints the reference (`<name>@sha256:…`) and the
   `cosign sign` command that signs it.

Log in with `docker login` first: ritsu, like cosign, reads the credentials Docker keeps. The
workflow that publishes this repository's modules,
[.github/workflows/modules.yml](https://github.com/i2y/ritsu/blob/main/.github/workflows/modules.yml),
checks a module, pushes it to ghcr.io, signs it with cosign without a key, and checks the signature
as a module that requires it would.

## Signatures

ritsu does not sign: cosign signs the digest `ritsu mod push` prints. In GitHub Actions, without a
key, with the permission `id-token: write`:

```console
$ cosign sign --yes ghcr.io/i2y/ritsu-holidays@sha256:…
```

With a key, it is `cosign sign --key cosign.key <name>@sha256:…`. A module that requires another
says which signature it takes with a `signature` line in its `ritsu.mod`. This is the line for this
repository's module:

```text
signature "ghcr.io/i2y/ritsu-holidays" by matching "^https://github[.]com/i2y/ritsu/" issuer "https://token.actions.githubusercontent.com"
```

| `signature "<name>" …` | What cosign checks with |
|---|---|
| `key "keys/acme.pub"` | `--key`, with the file read from the module's root |
| `key "keys/acme.pub" tlog off` | the same, taking a signature that is not in the transparency log |
| `by "<identity>" issuer "<issuer>"` | `--certificate-identity` and `--certificate-oidc-issuer` |
| `by matching "<regex>" issuer "<issuer>"` | `--certificate-identity-regexp` and `--certificate-oidc-issuer` |

- The string after `signature` is a module's name, or the start of names, ending in `/`
  (`"example.com/acme/"`).
- A module is held to the lines of the module at hand and of the modules that require it directly.
  Every line that applies must pass.
- `ritsu mod fetch` runs cosign from the PATH before it keeps a module, and records what it checked
  in the store. `ritsu check` runs no cosign: when the record lacks a line that applies, it stops
  and says to run `ritsu mod fetch`.
- ritsu checks the signatures of modules from registries alone. A line that covers a module from git
  is E309: narrow its start, or take the module from a registry.

## A site that lists modules

`ritsu mod site <list> --out <dir>` writes a site that lists modules, as
[the list of modules](modules/list/) on this site. It holds no account and no server: the modules
stay where they are published, and the command fetches every version, checks its digests, its
signature and `ritsu check`, and writes static pages. The list's page searches the modules and what
they hold. A module's page has what it holds, its README, the rules and calendars to try on the page,
what changed from the version before and, when it has a requirements file, why, its versions and files, each file's code with its lines numbered
and coloured, the steps that add it, what was checked, and a button that opens the version in the
playground. The list file has these lines:

| Line | What it says |
|---|---|
| `title`, `description` | The list's name, in the heading of its page and the header of every page, and the sentence under the heading; left out, each language has its own |
| `playground` | The URL of the page that opens a version; left out, this site's |
| `contribute` | An https URL the list's page links to, for those who want a module listed; left out, no link |
| `module "<name>"` | A module on a registry, every version of which is fetched and checked |
| `git "<URL>"`, `git "<URL>" dir "<dir>"` | A module shared by git, every tag of which is fetched and checked |
| `local "<dir>"` | A working tree not pushed yet, from the list's directory |
| `signature` | As in `ritsu.mod`: what checks the versions of the modules it covers, and not a module from git |

A `module`, `git` or `local` line can end with `as "<name>"`: the steps on the page add the module
under that name, and without it under the last part of its name. `--lang` picks the language of the
pages, and `--alternate <lang>=<path>` links each page to the same page of the site in another
language, by the path from this site's root. The command exits 1 when a version cannot be verified,
which its page says, and 2 when the list does not read or a registry or a repository cannot be
reached. A version that does not pass `ritsu check` is shown as it is, with what the check said.
The check is `ritsu check . --no-claims`: the claims of a `.geas` file are read and not run, because
running them would run the module's author's programs on the machine that builds the site.

### A list of your own on GitHub Pages

The site needs no server, so a list of your own modules can go out from your own repository. Put a
list file in it, `modules.list` here, and this workflow beside it, set
**Settings → Pages → Source** to **GitHub Actions**, and push them.

```text
# modules.list
title "Acme's modules"
description "The rules and calendars the Acme shop shares."
module "ghcr.io/acme/ritsu-pricing" as "pricing"
signature "ghcr.io/acme/" by matching "^https://github[.]com/acme/" issuer "https://token.actions.githubusercontent.com"
```

```yaml
# .github/workflows/ritsu-modules.yml
name: ritsu modules
on:
  push:
    branches: [main]
    paths: [modules.list, .github/workflows/ritsu-modules.yml]
  pull_request:
    paths: [modules.list]
  schedule:
    - cron: "17 21 * * *"
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: pages
  cancel-in-progress: false
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: i2y/ritsu@v0.46.0
        with:
          cosign: true
      - run: ritsu mod site modules.list --out site
      - uses: actions/upload-pages-artifact@v5
        with:
          path: site
  deploy:
    if: github.event_name != 'pull_request'
    needs: build
    runs-on: ubuntu-latest
    permissions:
      pages: write
      id-token: write
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v5
```

- It builds the site once a day as well, for the versions your modules have published since. A run
  that cannot read the list or verify a version publishes nothing, and the site stays as it was. A
  pull request builds the pages and stops there.
- The job that builds can only read: it writes pages from what the modules hold. Only the job that
  deploys can write to Pages.
- `cosign: true` puts cosign on the PATH for the `signature` lines. A list with none can leave it
  out.

The `run:` line is yours. For both languages from one root, make it two:

```text
ritsu mod site modules.list --out site --lang en --alternate ja=ja
ritsu mod site modules.list --out site/ja --lang ja --alternate en=..
```

To publish from a branch instead, commit what `--out` wrote at the root of what the branch publishes:
the command writes a `.nojekyll` there, so that `_site/` gets through.

Before you publish:

- **The pages hold everything in a module, code included.** With no credentials, a private module
  stops the command and nothing is written. A step that logs in to the registry makes it readable,
  and then its code is on the pages. A site on GitHub Pages can be private only for an organization
  on GitHub Enterprise Cloud.
- **A schedule can stop.** In a public repository, GitHub disables scheduled workflows after 60 days
  with no activity, and new versions stop showing. A list in the repository that publishes its
  modules stays active; elsewhere, turn the workflow on again from the Actions tab.
- **`<owner>.github.io` is one origin** for all of that owner's project sites. The pages write what a
  module holds as text and run none of it, but to list other people's modules, give the site a domain
  of its own, so that a mistake there cannot touch your other pages.

## What raising a version changes

`ritsu mod diff <old> [<new>]` compares two versions of a module. Each side is
`<module>@<version>` (from its git repository when the tree takes it from git), a directory, or `.`
(the working tree, when `<new>` is not given), and the modules either side reads are fetched when
they are not here. It lists the files added, removed and
changed and the changes of `require`. For every rule, dates file, calendar and gate in both versions,
it shows each input that gets a different answer, over the whole input space rather than a sample,
and says that the others get the same. For each changed file it shows the changes of what the file
shows the code that uses it, the ones that may break a user first. Here `pricing` 1.1.0 lowers the
fee of a parcel of up to 2,500g:

```console
$ cd crates/ritsu/tests/projects/modules/pricing
$ ritsu mod diff ../versions/pricing-1.0.0 .
ritsu mod diff: example.com/acme/pricing 1.0.0 → 1.1.0
  old: ../versions/pricing-1.0.0
  new: .
…
behavior:
  rules/order_total.rule (rulec): the same answers
  rules/shipping_fee.rule (rulec): the answers differ
…
      region = domestic  and  weight >=2001g <=2500g
        fee: 695 → 395
        rows: table fee_by_weight row 2 → table fee_by_weight row 1
        example: region=domestic, weight=2001
…
interface:
  rules/shipping_fee.rule (rulec, api): 10 changes, 0 of them may break a user
    added: /{python,typescript,javascript,rust,ruby,php,go,swift,java,connect}/enums/region/values/eu
…
```

A change that may break a user is a candidate: whether it does is for the user's check to say. A
removal or a change is one, and so is a new `constraint` or `never` line on a rule's inputs, which
refuses calls that used to work though the table answers as before. A `fed by` line is not a change
of the interface: the code that uses the file does not read it, and it is said on a line of its own.
Some changes break a user whatever its check says. An account or a transfer of a book that changed is
chobo's E050 or E051: the accounts and the holds already in a database keep the old definition. A
value of an enum or a field of a message whose number on the Connect wire moved is E310: a message
one version writes, the other reads as another value or another field, and neither fails. A rule
that does not write `implements` is numbered in the order it writes things, which is why `pricing`
1.1.0 adds `eu` after the other regions. A rule that writes `implements proto "<file>" <Service>`
has a contract's numbers, which move only when the contract moves them, and it is compared when a
`.proto` of the module changed even if the rule did not. `ritsu mod diff` says these among the
changes of the interface, and `ritsu mod report` counts them as errors. `rulec diff`, `koyomi diff` and `sekisho diff` name a
version in the same ways, and two more: `@<name>/<path>` is a file of a dependency at the version
the lock pins, and `@<name>@<version>/<path>` the file at that version of the module.

A `.req` has no interface: it answers nothing, and no file of a user reads it. It says why the
answers are what they are, so `ritsu mod diff` says each requirement that moved: its text before and
now, when it is in force, the `decided` lines the new version adds, what it is read from, what meets
and checks it, and who looked. The difference in a source's text comes from the copies the records
keep under `reviewed/`. These are the words of the module's file: ritsu checks that what they were
written against has not moved since someone looked, not that they are right. The Changes tab of
[`osha-penalties` 1.2.0](modules/list/ghcr.io/i2y/ritsu-osha-penalties/1.2.0/) shows one, for
the amendment of 2025.

`ritsu mod report --base <revision>` answers what a pull request that raises a dependency asks. It
reads your files as the head has them, with the dependencies of the base and then with those of the
head, and fetches the modules both locks pin. It says which diagnostics the head adds and removes,
and, for each module that changed, its diff, which answers of your own files change, and which
lines of your flows and kaname modules read an answer that changes: dandori and kaname have no diff
of behavior, so those lines are listed, not compared. A module whose tag moved, the same version
with other files, comes first. The markdown is a comment for the pull request:

```yaml
# .github/workflows/ritsu-mod-report.yml
name: ritsu mod report
on:
  pull_request:
    paths: [ritsu.mod, ritsu.lock]
permissions:
  contents: read
  pull-requests: write
jobs:
  report:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - uses: i2y/ritsu@v0.46.0
      - name: Report what raising the dependencies changes
        env:
          GH_TOKEN: ${{ github.token }}
          PR: ${{ github.event.pull_request.number }}
        run: |
          code=0
          ritsu mod report --base "origin/$GITHUB_BASE_REF" --format markdown > report.md || code=$?
          ritsu mod report --base "origin/$GITHUB_BASE_REF" --format json > report.json || true
          if [ -s report.md ]; then gh pr comment "$PR" --body-file report.md; fi
          exit "$code"
      - uses: actions/upload-artifact@v7
        if: always()
        with:
          name: ritsu-mod-report
          path: report.json
```

- `fetch-depth: 0` fetches the history, so that `origin/<base>` can be read.
- `ritsu mod report` exits 1 when the head has errors, and 2 when it cannot compare a module that
  changed. Add `--fail-on-change` to fail when answers change too.
- For a module that is not at the root of the repository, give its directory in `paths` and to
  `--root`.
- A pull request from a fork gets a token that cannot write, and no comment.
- `--lang ja` writes the report in Japanese.

## Libraries

A module is published for npm, PyPI and Go with `ritsu gen --library`:
[Publishing libraries](libraries.md).
