#!/bin/sh
# The Context Mapper CLI 6.12.0 (PLAN 0.4), fetched from Maven Central, held to its SHA-256 and
# opened into tools/cml/context-mapper-cli-6.12.0. sakai's test does not run the CLI itself: its
# `cm validate` reads the syntax only (DESIGN 0.4). It runs Validate.java on the jars of lib/,
# which runs the semantic checks of Context Mapper's Xtext validator. Neither the zip nor what
# it opens into is in git.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
zip=context-mapper-cli-6.12.0.zip
want=96579d57a5afa110d7b1363463cc576a2494cbbf480c37620aa09960c7779ce0

digest() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

if [ ! -f "$here/$zip" ]; then
  curl -fsSL -o "$here/$zip.part" "https://repo1.maven.org/maven2/org/contextmapper/context-mapper-cli/6.12.0/$zip"
  mv "$here/$zip.part" "$here/$zip"
fi
got=$(digest "$here/$zip")
if [ "$got" != "$want" ]; then
  echo "$zip: the SHA-256 is $got, not $want" >&2
  rm -f "$here/$zip"
  exit 1
fi
rm -rf "$here/context-mapper-cli-6.12.0"
(cd "$here" && unzip -q "$zip")
ls "$here/context-mapper-cli-6.12.0/lib" | wc -l | sed 's/^ */jars in lib: /'
