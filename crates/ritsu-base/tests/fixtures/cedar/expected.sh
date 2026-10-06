#!/bin/sh
# What the official Cedar CLI makes of the files beside this script. tests/cedar.rs holds
# ritsu-base's readers and writers to these outputs byte for byte, so the test needs no Cedar.
#
#   $ cargo install --root <dir> cedar-policy-cli@4.13.0   (or the release's binary for the machine:
#     https://github.com/cedar-policy/cedar/releases/tag/cedar-policy-cli-v4.13.0)
#   $ CEDAR=<dir>/bin/cedar sh expected.sh
#
# The files under this directory are written for ritsu, except upstream/: those are copied from
# the Cedar repository at v4.13.0 (Apache-2.0; upstream/LICENSE and upstream/NOTICE are its
# license and notice): upstream/policies/formatter-*.cedar from cedar-policy-formatter/tests/,
# upstream/policies/sandbox_*-*.cedar and upstream/schemas/sandbox_* from
# cedar-policy-cli/sample-data/ (schema.cedarschema.json renamed <sandbox>.schema.json).
#
# In policies/ and upstream/policies/, for each <name>.cedar:
#   <name>.json   translate-policy --direction cedar-to-json
#   <name>.fmt    format
#   <name>.fmt40  format --line-width 40 --indent-width 4
# and written/<name>.cedar (what write_policies writes for it; CEDAR_BLESS=1 cargo test writes
# them) must be formatted already (format --check) and translate to the same <name>.json.
# In schemas/ and upstream/schemas/:
#   for each <name>.cedarschema:  <name>.json (translate-schema --direction cedar-to-json)
#                                 <name>.back.cedarschema (that JSON, json-to-cedar)
#   for each <name>.schema.json:  <name>.schema.cedarschema (json-to-cedar)
#                                 <name>.schema.back.json (that, cedar-to-json)
# invalid/<name>.cedar and invalid-schemas/<file>: the CLI must not read them
# (translate-policy, or translate-schema in the direction that reads the file); its message goes
# to <name>.err and <file>.err for people to read.
set -eu
CEDAR=${CEDAR:-cedar}
cd "$(dirname "$0")"
"$CEDAR" --version | grep -q '^cedar-policy-cli 4\.13\.0$' || { echo "expected cedar-policy-cli 4.13.0" >&2; exit 1; }
export CEDAR_ERROR_FORMAT=plain NO_COLOR=1

policies() {
  for f in "$1"/policies/*.cedar; do
    [ -e "$f" ] || continue
    b=${f%.cedar}
    "$CEDAR" translate-policy --direction cedar-to-json -p "$f" > "$b.json"
    "$CEDAR" format -p "$f" > "$b.fmt"
    "$CEDAR" format --line-width 40 --indent-width 4 -p "$f" > "$b.fmt40"
  done
  for f in "$1"/written/*.cedar; do
    [ -e "$f" ] || continue
    n=$(basename "$f" .cedar)
    "$CEDAR" format --check -p "$f" > /dev/null || { echo "$f is not formatted" >&2; exit 1; }
    "$CEDAR" translate-policy --direction cedar-to-json -p "$f" | cmp -s - "$1/policies/$n.json" || { echo "$f does not read as $1/policies/$n.cedar" >&2; exit 1; }
  done
}

schemas() {
  for f in "$1"/schemas/*.cedarschema; do
    [ -e "$f" ] || continue
    case "$f" in *.back.cedarschema|*.schema.cedarschema) continue ;; esac
    b=${f%.cedarschema}
    "$CEDAR" translate-schema --direction cedar-to-json -s "$f" > "$b.json"
    "$CEDAR" translate-schema --direction json-to-cedar -s "$b.json" > "$b.back.cedarschema"
  done
  for f in "$1"/schemas/*.schema.json; do
    [ -e "$f" ] || continue
    b=${f%.json}
    "$CEDAR" translate-schema --direction json-to-cedar -s "$f" > "$b.cedarschema"
    "$CEDAR" translate-schema --direction cedar-to-json -s "$b.cedarschema" > "$b.back.json"
  done
}

policies .
policies upstream
schemas .
schemas upstream

for f in invalid/*.cedar; do
  [ -e "$f" ] || continue
  b=${f%.cedar}
  if "$CEDAR" translate-policy --direction cedar-to-json -p "$f" > /dev/null 2> "$b.err"; then
    echo "the CLI reads $f" >&2; exit 1
  fi
done

for f in invalid-schemas/*; do
  [ -e "$f" ] || continue
  case "$f" in *.err) continue ;; esac
  case "$f" in
    *.json) d=json-to-cedar ;;
    *) d=cedar-to-json ;;
  esac
  if "$CEDAR" translate-schema --direction $d -s "$f" > /dev/null 2> "$f.err"; then
    echo "the CLI reads $f" >&2; exit 1
  fi
done
echo ok
