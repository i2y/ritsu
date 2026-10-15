#!/usr/bin/env bash
# The editor and the Markdown of the page in the browser, as one file the page loads:
# website/docs/playground/libraries.js (DESIGN 8.7).
#
# It is committed, as ritsu.wasm is, so that building the site needs no Node, and the page loads
# nothing from anywhere else. It is made from website/tools/libraries: package.json names the
# packages at exact versions, each out for two weeks or more when it was locked, and
# package-lock.json pins every package they bring at such a version too (`npm install --before`);
# osv-scanner reads the lock with the repository's other lockfiles (.github/workflows/audit.yml).
# Run this again after changing the packages or entry.js:
#
#   $ website/tools/make_libraries.sh
#
# It installs the lock as it is (`npm ci`, without the packages' scripts) into
# website/tools/libraries/node_modules, which is not committed, and bundle.mjs bundles entry.js with
# esbuild into one minified ES module, with the name, version, license and source of every package
# it holds above the code, and the text of each license as the package gives it.
# crates/ritsu/tests/playground.rs holds that list to the lock, and the notices (THIRD_PARTY_NOTICES)
# to the texts.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
(cd "$here/libraries" && npm ci --ignore-scripts --no-audit --no-fund)
node "$here/libraries/bundle.mjs" "$here/../docs/playground/libraries.js"
ls -l "$here/../docs/playground/libraries.js"
