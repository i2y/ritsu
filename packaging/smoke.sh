#!/bin/sh
# Runs each of the seven names of ritsu once on an example of its language, and `ritsu check` on a
# project of them all, to show that a build, an archive or a package starts under every name and
# does what the name says. The three languages that read others (dandori, yuen, sakai) are given
# examples that read another language, so a binary whose ports were not joined fails here.
# release.yml runs it on the unpacked archive, linux.sh on each package after it is installed, and
# crates/ritsu/tests/release.rs on the binary of the test.
#
#   sh packaging/smoke.sh <the directory the links are in, or - for the PATH> <the crates/ directory>
set -eu

dir=$1
crates=$2

run() {
  name=$1
  shift
  if [ "$dir" = - ]; then "$name" "$@"; else "$dir/$name" "$@"; fi
}

# say what is being run, run it, and keep its output out of the way unless it fails
step() {
  name=$1
  shift
  if out=$(run "$name" "$@" 2>&1); then
    echo "ok   $name $*"
  else
    echo "FAIL $name $*" >&2
    echo "$out" >&2
    exit 1
  fi
}

# the languages that read no other
step rulec check "$crates/rulec/tests/corpus/"
step koyomi check "$crates/koyomi/examples/net30.cal"
step chobo check "$crates/chobo/examples/inventory/inventory.book"
step geas explain --all
# the languages that read others: a flow that uses rules, a requirement that names a rule's source,
# a map that crosses into rules, calendars and flows
step dandori check "$crates/dandori/examples/hotel/temporal/hotel.flow"
step yuen check --root "$crates/yuen/tests/fixtures/rulec" "$crates/yuen/tests/fixtures/rulec"
# (the first map under sakai's examples: its name follows the language the example is written in)
map=$(ls "$crates"/sakai/examples/*/*.ctx | head -n 1)
step sakai check "$map"
# ritsu itself: a project with a file of each language, each checked with its language's check and
# then across them (the first project under ritsu's tests, whatever language it is written in)
project=$(ls -d "$crates"/ritsu/tests/projects/*/ | head -n 1)
step ritsu check "${project%/}"
