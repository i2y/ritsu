#!/bin/sh
# Writes the archive of one release for one platform: `ritsu`, a link to it named for each of the
# seven languages, the two licenses, and THIRD_PARTY_NOTICES, the notices and licenses of what the
# binary holds from others (DESIGN 2.3, 13.2). release.yml runs it on each of the four platforms,
# and crates/ritsu/tests/release.rs on the binary of the test.
#
#   sh packaging/archive.sh v0.23.0 aarch64-apple-darwin target/aarch64-apple-darwin/release/ritsu dist/
#
# The archive is flat, with the links beside the binary. They are relative (`rulec -> ritsu`), so
# the directory they are unpacked into can be anywhere, and unpacking into a directory on the PATH
# is the whole install: `tar -xzf ritsu-v0.23.0-aarch64-apple-darwin.tar.gz -C ~/.local/bin`. That
# puts LICENSE-MIT, LICENSE-APACHE and THIRD_PARTY_NOTICES there too;
# `--exclude 'LICENSE-*' --exclude THIRD_PARTY_NOTICES` leaves them in the archive.
set -eu

tag=$1
target=$2
bin=$3
out=$4

# The names of the links: the seven languages (crates/ritsu/src/cli.rs says the same).
languages="rulec dandori koyomi chobo geas yuen sakai"
# The licenses and the notices sit at the root of the repository, one up from this script.
root=$(cd "$(dirname "$0")/.." && pwd)
licenses="LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$bin" "$stage/ritsu"
chmod 755 "$stage/ritsu"
for l in $languages; do
  ln -s ritsu "$stage/$l"
done
for f in $licenses; do
  cp "$root/$f" "$stage/$f"
done

mkdir -p "$out"
# shellcheck disable=SC2086
tar -czf "$out/ritsu-$tag-$target.tar.gz" -C "$stage" ritsu $languages $licenses
echo "$out/ritsu-$tag-$target.tar.gz"
