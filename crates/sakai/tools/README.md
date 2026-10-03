# The tools sakai's tests run

`sakai build` writes the settings of four import linters, and `sakai export cml` writes Context
Mapper's CML. sakai itself runs none of these tools. Its tests do: `tests/imports.rs` runs each
linter on the settings sakai writes for the example, and `tests/cml.rs` runs Context Mapper's
validator on the CML. A test whose tool is not here prints `SKIP: <why>` and passes.

Everything these scripts fetch stays out of git (see `.gitignore`); only the files that pin the
versions are kept.

| Tool | Version | Put it in place | The test finds it at |
|---|---|---|---|
| import-linter (grimp 3.17) | 2.15 | `uv venv --python 3.13 tools/.venv && uv pip install --python tools/.venv/bin/python --require-hashes -r tools/requirements.txt` | `SAKAI_LINT_IMPORTS`, else `tools/.venv/bin/lint-imports` |
| dependency-cruiser, TypeScript | 16.10.4, 5.9.3 | `npm ci --prefix tools` | `SAKAI_DEPCRUISE`, else `tools/node_modules/.bin/depcruise` |
| ArchUnit, JUnit Platform Console | 1.5.1, 6.1.3 | `tools/java/fetch.sh` (checks each jar's SHA-256) | `SAKAI_ARCHUNIT_LIB`, else `tools/java/lib`; Java from `SAKAI_JAVA` and `SAKAI_JAVAC`, else `JAVA_HOME`, else the PATH |
| go-arch-lint | v1.19.0 | `tools/go/install.sh` (builds with `-trimpath`) | `SAKAI_GO_ARCH_LINT`, else `tools/go/bin/go-arch-lint`; go from `SAKAI_GO`, else the PATH |
| Context Mapper CLI | 6.12.0 | `tools/cml/fetch.sh` (checks the zip's SHA-256) | `SAKAI_CML_LIB`, else `tools/cml/context-mapper-cli-6.12.0/lib` |

`tools/requirements.txt` is `tools/requirements.in` compiled with hashes:
`uv pip compile --python 3.13 --generate-hashes --no-header tools/requirements.in -o tools/requirements.txt`.

Two things to know:

- dependency-cruiser 16 reads TypeScript 2 up to, not including, 6. With TypeScript 6 or later
  installed, it reads no `.ts` file and passes every one of them in silence. The test fails
  rather than skips when `depcruise --info` does not show TypeScript as read. dependency-cruiser
  17 and 18 do not support node 23, which is why the version is 16.10.4.
- `tools/cml/Validate.java` runs Context Mapper's Xtext validator with every check. The CLI's own
  `cm validate` reports the syntax only, and exits 0 whatever it finds.

The suite's tools (rulec 0.22 or later, koyomi, chobo, dandori) are found by `SAKAI_RULEC`,
`SAKAI_KOYOMI`, `SAKAI_CHOBO` and `SAKAI_DANDORI`, else on the PATH; `tests/examples.rs` checks the
files the example copied from them with each one's own `check`. buf (`SAKAI_BUF`, else the PATH)
lints the two protos written for the example and is compared with sakai's proto reader.
