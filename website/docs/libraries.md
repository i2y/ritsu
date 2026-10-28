# Publishing libraries

`ritsu gen` writes a module as a package for each language, for an application to run: the
module's code and the code of every module it requires (a dependency's under `deps/<name>/`),
with the runtime at the exact versions ritsu was tested with. `ritsu gen --library` writes each
package as a library instead, in the form its registry takes: an npm package, a Python
distribution, a Go module. A library holds the code of its own module. It imports what a
dependency holds from that dependency's library, by its name, and its manifest asks for each
library and for the runtime by a range. Others then call the module's rules, dates, books and
workflows from their own code, without generating them. Without `--library`, `ritsu gen` writes
what it always has.

| | `ritsu gen` | `ritsu gen --library` |
|---|---|---|
| For | an application, run as it is | other code, through a registry |
| A dependency's code | inside, under `deps/<name>/` | in the dependency's library, imported by its name |
| The runtime and the dependencies | at the exact versions tested | by ranges from those versions |
| The npm package | `"private": true` | published, built into `dist/` |

The examples on this page are three modules of ritsu's tests,
[crates/ritsu/tests/projects/modules](https://github.com/i2y/ritsu/tree/main/crates/ritsu/tests/projects/modules):
`tax` holds the VAT rates, `pricing` the shipping fees and the order totals, which apply `tax`'s
rate, and `shop` an order flow and a basket, which `pricing` prices.

## Name the libraries

A module names its library in each language with a `library` line in its ritsu.mod:

<!-- pricing/ritsu.mod -->
```text
# The prices of the Acme shop: shipping fees, and order totals with the VAT rate of tax.
module example.com/acme/pricing
version 1.1.0
description "Shipping fees and order totals of the Acme shop"
license "MIT OR Apache-2.0"
source "https://github.com/acme/pricing"

library typescript "@acme/pricing"
library python "acme-pricing"
library go "example.com/acme/pricing-go"

require tax path "../tax"
```

- `library typescript|python|go "<name>"` is the package's name in that language's registry: the
  npm package, the Python distribution, the path of the Go module. Python imports the
  distribution `acme-pricing` as `acme_pricing`: the name in lower case, each run of `-`, `_` and
  `.` made one `_`. From version 2, the path of the Go module ends in `/v2`, as Go requires, and
  the code that imports it says so.
- `version` is the version of every library, and `--library` needs it. Python's versions (PEP 440)
  take a pre-release only as `alpha`, `beta` or `rc` with a number: `1.0.0-rc.1` is `1.0.0rc1`,
  and `1.0.0-dev.1` stops the run.
- `description` and `license` go into package.json and pyproject.toml, and `source` into
  package.json's `repository`, which npm's provenance checks ([To npm](#to-npm)), and into the
  `Repository` of pyproject.toml's `[project.urls]`.

## Generate

<!-- run in pricing -->
```console
$ ritsu gen --library --out ../out/pricing
generated: ../out/pricing/typescript/rules/order_total.ts
…
generated: ../out/pricing/python/pyproject.toml
…
generated: ../out/pricing/go/go.mod
```

`ritsu gen --library` writes a library for each language the module has a `library` line for,
in `typescript/`, `python/` and `go/` under `--out`; `--target typescript|python|go` writes one.
A library is the whole module, all but what its `exclude` lines leave out, so `--library` takes
no paths, and no `--name` or `--module`: the `library` lines name the libraries. `--check` says
what differs from what is there, as it does without `--library`.

## Dependencies come from their libraries

`shop` requires `pricing`, and its code imports `pricing`'s rules from `pricing`'s library, by the
name `pricing`'s ritsu.mod gives it in each language:

<!-- out/shop/typescript/glue/basket.ts -->
```ts
import * as rule_pricing_order_total from "@acme/pricing/rules/order_total";
```

<!-- out/shop/python/acme_shop/glue/basket.py -->
```python
from acme_pricing.rules import order_total as _rule_pricing_order_total
```

<!-- out/shop/go/glue/basket/basket.go -->
```go
pricingordertotal "example.com/acme/pricing-go/rules/ordertotal"
```

A manifest names only what the library's code imports. `pricing`'s order total applies `tax`'s
rule, which is compiled into it, so `pricing`'s code imports nothing of `tax`, and its manifests
do not name `tax`.

Every dependency still needs a library of its own, or `--embed`. When the ritsu.mod of a
dependency has no line for a language, the run stops and says which:

<!-- run in pricing, with no library lines in tax's ritsu.mod -->
```console
$ ritsu gen --library --out ../out/pricing
error: the ritsu.mod of the dependency `tax` (example.com/acme/tax) has no `library typescript` line, and a library reads its dependencies' parts from their libraries: write `library typescript "<name>"` there, or give `--embed tax` to hold the parts in this package's deps/
```

With `--embed tax`, `pricing`'s libraries hold `tax`'s code under `deps/tax/`, as a package
without `--library` does. Then `shop`, which reaches `tax` only through `pricing`, reads `tax`
from `pricing`'s library, and `--embed tax` in `shop` stops the run: a module that a package
reaches only through a library is that library's to hold.

### Ranges

A library asks for each library it imports by a range from the version the tree of modules
chose, and for the runtime by a range from the version ritsu was tested with. A range goes up to
the next major version, or for a version `0.y.z`, to the next minor one:

| The version chosen | npm | Python | Go |
|---|---|---|---|
| `1.1.0` | `^1.1.0` | `>=1.1.0,<2.0.0` | `v1.1.0` |
| `0.17.9` | `^0.17.9` | `>=0.17.9,<0.18.0` | `v0.17.9` |
| `0.0.3` | `>=0.0.3 <0.1.0` | `>=0.0.3,<0.1.0` | `v0.0.3` |

npm's `^0.0.3` would take `0.0.3` alone, so that range is written out. Go writes the version
itself: Go reads a `require` as the lowest version the module takes, and builds with the highest
of those its modules ask for.

Without `--library`, a package pins the exact versions instead. An application is where the
versions end, and it runs what it was tested with. A library is one dependency of an
application among others: if two libraries pinned two versions of the same runtime, Python could
not install both, and npm would install two copies. A library gives a range, and the
application's lock file chooses within it. `shop` imports `pricing` and Temporal's SDK:

<!-- out/shop/typescript/package.json -->
```json
  "dependencies": {
    "@temporalio/activity": "^1.24.0",
    "@temporalio/client": "^1.24.0",
    "@temporalio/common": "^1.24.0",
    "@temporalio/worker": "^1.24.0",
    "@temporalio/workflow": "^1.24.0"
  },
  "peerDependencies": {
    "@acme/pricing": "^1.1.0"
  },
```

<!-- out/shop/python/pyproject.toml -->
```toml
dependencies = [
  "acme-pricing>=1.1.0,<2.0.0",
  "temporalio>=1.33.0,<2.0.0",
]
```

<!-- out/shop/go/go.mod -->
```text
require (
	example.com/acme/pricing-go v1.1.0
	go.temporal.io/api v1.63.5
	go.temporal.io/sdk v1.49.0
)
```

On npm, the libraries a library imports are `peerDependencies`, so that an application installs
one copy of `pricing` for itself and for every library that imports it.

## Units cross libraries as one type

`shop`'s amounts of money are `pricing`'s: its units module takes the type from `pricing`'s
library and hands it on under the same name.

<!-- out/shop/typescript/units.ts -->
```ts
import type { GBPc } from "@acme/pricing/units";

export type { GBPc }; // money[GBPc]
```

<!-- out/shop/python/acme_shop/units.py -->
```python
from acme_pricing.units import GBPc as GBPc  # money[GBPc]
```

<!-- out/shop/go/units/units.go -->
```go
import pricinggounits "example.com/acme/pricing-go/units"

// GBPc is an amount of money[GBPc], the type of example.com/acme/pricing-go/units.
type GBPc = pricinggounits.GBPc
```

An amount `shop`'s code gives passes to `pricing`'s rules as it is, and the type checker of each
language still keeps an amount of another unit out. If two libraries with no dependency between
them both held the same unit, a package that imports both would have two types of it, so the
run stops; make one library depend on the other, or generate without `--library`.

## What each registry gets

### npm

<!-- out/pricing/typescript/package.json -->
```json
{
  "name": "@acme/pricing",
  "version": "1.1.0",
  "description": "Shipping fees and order totals of the Acme shop",
  "license": "MIT OR Apache-2.0",
  "repository": {
    "type": "git",
    "url": "https://github.com/acme/pricing"
  },
  "type": "commonjs",
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "default": "./dist/index.js"
    },
    "./*": {
      "types": "./dist/*.d.ts",
      "default": "./dist/*.js"
    }
  },
  "files": [
    "dist"
  ],
  "scripts": {
    "prepare": "tsc -p tsconfig.json"
  },
  "devDependencies": {
    "typescript": "7.0.2"
  }
}
```

- The library is published built. npm runs `prepare` (`tsc` with the tsconfig.json beside it)
  when it installs the directory and before it packs or publishes it, and the package holds
  `dist/` alone: the JavaScript and its declarations. The TypeScript itself would not do, as Node
  does not strip the types of a `.ts` file under node_modules.
- It is CommonJS, which an ES module imports too. The generated code's relative imports have no
  extensions, which Node's ES modules would need.
- The `description` is the module's `description` line, which npm shows on the package's page.
- The first key, `"//"` (left out above), holds the sentence that says ritsu generated the file,
  `Code generated by ritsu <version>. DO NOT EDIT.`, as JSON has no comments to hold it. npm
  passes the key over. `ritsu gen` reads the sentence from this key, as it reads it from the head
  comment of the other files it writes.
- `optional-modules.d.ts` declares, as modules with no types, the SDKs that the code loads only
  when it is asked to (a workflow's agent loads its model's SDK with `await import(…)`), so that
  the library builds without them. An application that asks for one installs it.

### PyPI

<!-- out/pricing/python/pyproject.toml -->
```toml
[project]
name = "acme-pricing"
version = "1.1.0"
description = "Shipping fees and order totals of the Acme shop"
license = "MIT OR Apache-2.0"
requires-python = ">=3.10"
dependencies = [
]

[project.urls]
Repository = "https://github.com/acme/pricing"

[build-system]
requires = ["setuptools>=77"]
build-backend = "setuptools.build_meta"
```

- `requires-python` is the one a package without `--library` has. setuptools reads `license` as
  an SPDX expression (PEP 639) from 77 on.
- `Repository` is the module's `source` line, written only when the module has one. When a
  trusted publisher ([To PyPI](#to-pypi)) uploads the project from GitHub Actions in that
  repository, PyPI shows the URL as verified.

### Go

<!-- out/shop/go/go.mod -->
```text
module example.com/acme/shop-go

go 1.26.0
```

- The `go` line is the oldest Go release that is still supported (Go supports the two newest
  major releases), or higher if a runtime module the library requires asks for more.
- ritsu writes no go.sum. The code that uses the library records the sums of its modules in its
  own go.sum when it runs `go get`.
- The generated directory does not build by itself: its go.mod names what the library imports,
  not what those modules import in turn, and Go wants both. Do not run `go mod tidy` there: it
  writes the rest into go.mod, `ritsu gen --check` then says go.mod is out of date, and the next
  `ritsu gen` takes them out again. To build the library before it is tagged, require it from
  another module, with a `replace` to the directory, and run
  `go vet -mod=mod example.com/acme/pricing-go/...` in that module.
- In the CI of the library's own repository, build it with `go build -mod=mod ./...` (or vet it
  with `go vet -mod=mod ./...`). Go writes what the build needs into the go.mod and go.sum of
  the CI's checkout; commit neither.

## Publish

Publish the libraries in the order the modules require each other: `tax`, then `pricing`, then
`shop`. A library asks for each dependency from the version the tree chose, so that version has
to be in the registry first; npm installs it even to build the library.

The workflows below run in the module's repository (its `source`), on a tag of the module's
version. They install ritsu with its GitHub Action and generate the library there. ritsu's tests
build the libraries as these registries' tools do, but do not run the workflows.

### To npm

npm attaches provenance to a package, a signed statement of the repository and the workflow that
built it, when the package is published from GitHub Actions with an OIDC token
(`id-token: write`). It requires package.json's `repository` to be that repository, exactly: the
library takes it from the `source` line.

With a trusted publisher set for the package on npmjs.com (npm 11.5.1 or later, Node 22.14.0 or
later), the workflow needs no token, and npm adds provenance on its own:

```yaml
name: Publish the npm package
on:
  push:
    tags: ["v*"]
jobs:
  publish:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      id-token: write
    steps:
      - uses: actions/checkout@v6
      - uses: i2y/ritsu@v0.36.0
      - uses: actions/setup-node@v6
        with:
          node-version: "24"
          registry-url: "https://registry.npmjs.org"
          package-manager-cache: false
      - run: ritsu mod fetch
      - run: ritsu gen --library --target typescript --out lib
      - run: npm install
        working-directory: lib/typescript
      - run: npm publish
        working-directory: lib/typescript
```

`ritsu mod fetch` fetches the modules the lock pins that are not there yet. `npm install` installs
TypeScript and the libraries the package imports, and builds `dist/`; `npm publish` builds it again
and publishes it. With a token instead of a trusted publisher, the last step is the one below
(a package's first publish needs `--access public`):

```yaml
      - run: npm publish --provenance --access public
        working-directory: lib/typescript
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

### To PyPI

PyPI's trusted publishers take a release from a workflow with no token. Set one for the project on
PyPI, with the repository, the file name of the workflow and the environment `pypi`; for a project
not yet on PyPI, a pending publisher creates it at the first upload. As the Python Packaging
Authority's guide has it, one job builds the distributions and another, which alone gets the OIDC
token, publishes them:

```yaml
name: Publish the Python package
on:
  push:
    tags: ["v*"]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false
      - uses: i2y/ritsu@v0.36.0
      - uses: actions/setup-python@v6
        with:
          python-version: "3.x"
      - run: ritsu mod fetch
      - run: ritsu gen --library --target python --out lib
      - run: python3 -m pip install build --user
      - run: python3 -m build lib/python
      - uses: actions/upload-artifact@v5
        with:
          name: python-package-distributions
          path: lib/python/dist/
  publish:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: pypi
      url: https://pypi.org/p/acme-pricing
    permissions:
      id-token: write
    steps:
      - uses: actions/download-artifact@v6
        with:
          name: python-package-distributions
          path: dist/
      - uses: pypa/gh-action-pypi-publish@release/v1
```

The action uploads attestations of the distributions with them, by default.

### To Go

Go takes a module from the repository at its path, by a tag of its version: there is nothing to
upload. Generate the library into a clone of that repository and tag it there. Below, `lib/go`
is a clone of the repository of `example.com/acme/pricing-go`:

```console
$ ritsu gen --library --target go --out lib
$ cd lib/go
$ git add -A
$ git commit -m "pricing-go v1.1.0"
$ git tag v1.1.0
$ git push origin HEAD v1.1.0
$ GOPROXY=proxy.golang.org go list -m example.com/acme/pricing-go@v1.1.0
```

The last command has Go's module proxy fetch the version, as Go's guide to publishing does. When
the module is in a subdirectory of its repository, the subdirectory is in the module's path and
begins its tags: the module `example.com/acme/pricing/go`, in the directory `go` of the
repository `example.com/acme/pricing`, is tagged `go/v1.1.0`. Do not move a tag once it is
pushed: Go checks each download of a version against the first, and takes a different copy as a
security error.

## What the registries' tools write

tsc writes `dist/` beside a library, setuptools `build/` and `<name>.egg-info/`, and Go `go.sum`.
With `--library`, `ritsu gen --check` and the next `ritsu gen` leave these as they are, at the
root of each library's directory, though tsc and setuptools copy the first line of ritsu's files,
which says ritsu generated them.

## What a library cannot do

- **Take a contribution to the rule of a library it imports.** A library's code is built with the
  contributions its module has when it is generated, and takes none from the code that uses it.
  When a contribution of the package adds to a rule that a library holds, the run stops. Generate
  without `--library`, so that the package holds the rule with the contribution in it, or move the
  contribution into the library's module.
- **Pass kaname's types for a book between libraries.** The types kaname hands to a book
  (`glue/books/`) are the package's own. A library with the same book has its own types of it,
  which Python and Go take as different types (TypeScript compares types by their shape, and takes
  them as one), and its own `KanameError`. To pass the values of a book between modules, generate
  them as one package, without `--library`.
- **Hold one unit in two libraries that do not depend on each other**:
  [Units cross libraries as one type](#units-cross-libraries-as-one-type) says why, and what to do.
