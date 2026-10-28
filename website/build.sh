#!/usr/bin/env bash
# Build ritsu's site into build/: ritsu's own pages in both languages (build/ and build/ja), then
# the site of each language below, in both of its languages, each built in its own directory by its
# own build.sh and copied to build/<language> (and build/<language>/ja).
#
# Always the whole of it, in this order: ritsu's English build cleans build/, so a language's site
# put there before it would be lost, and the Japanese build then writes build/ja.
set -eu
cd "$(dirname "$0")"
# The sites of the languages, each a directory here with its own zensical.toml and build.sh, built
# into <directory>/build and published at /ritsu/<directory>/.
sites=(rulec dandori chobo)
./sync.sh
# The page cache is keyed on the Markdown, not on what renders it, so a change to a lexer would leave
# every cached page as it was.
rm -rf .cache
# Pygments colours the code with the lexers of tools/ (knlexer.py the ```kn fences, gatelexer.py the
# ```gate fences, cedarlexer.py Cedar's ```cedar and ```cedarschema) and the ones in the tools/ of
# rulec's, dandori's and chobo's sites, which colour those sites too, the ```rule, ```flow and ```book
# fences. Each is imported only by being named as a Markdown extension in the configurations, so each
# has to be importable; the sites' own builds below keep their PYTHONPATH (chobo's puts ../tools on
# it for gatelexer.py).
lexers="$PWD/tools:$PWD/rulec/tools:$PWD/dandori/tools:$PWD/chobo/tools"
PYTHONPATH="$lexers${PYTHONPATH:+:$PYTHONPATH}" .venv/bin/zensical build
PYTHONPATH="$lexers${PYTHONPATH:+:$PYTHONPATH}" .venv/bin/zensical build -f zensical.ja.toml
for site in "${sites[@]}"; do
  "$site/build.sh"
  rm -rf "build/$site"
  cp -R "$site/build" "build/$site"
done
# The list of modules (DESIGN 17.17): `ritsu mod site` writes it from modules.list, in English and in
# Japanese. It takes ritsu: $RITSU, else ritsu on the PATH (docs.yml builds it from the commit);
# without either, the site is built without the list, and this says so.
ritsu="${RITSU:-$(command -v ritsu || true)}"
if [ -n "$ritsu" ]; then
  "$ritsu" mod site modules.list --out build/modules/list --lang en
  "$ritsu" mod site modules.list --out build/ja/modules/list --lang ja
fi
echo "site: build/ (en) + build/ja (ja)"
for site in "${sites[@]}"; do echo "  $site: build/$site (en) + build/$site/ja (ja)"; done
if [ -n "$ritsu" ]; then
  echo "  modules: build/modules/list (en) + build/ja/modules/list (ja)"
else
  echo "  modules: not built, as no ritsu was found (set RITSU, or put ritsu on the PATH)"
fi
