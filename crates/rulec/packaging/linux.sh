#!/bin/sh
# Builds the .deb and the .rpm of one release from its static Linux binary, and installs each
# on the distribution it is for before either is kept (DESIGN §15.158): a package that does
# not install is worse than none. release.yml runs it on both Linux runners, and ci.yml on
# every push, so a release is not the first time it runs.
#
#   sh packaging/linux.sh 0.9.0 x86_64-unknown-linux-musl target/x86_64-unknown-linux-musl/release/rulec dist/
#
# It needs docker and nothing else. nfpm writes the packages from packaging/nfpm.yaml and
# runs from its image, pinned by digest as CI pins its other tools.
set -eu

version=$1
target=$2
bin=$3
out=$4

case "$target" in
  x86_64-unknown-linux-musl) arch=amd64 rpmarch=x86_64 ;;
  aarch64-unknown-linux-musl) arch=arm64 rpmarch=aarch64 ;;
  *) echo "no package is built for $target" >&2; exit 2 ;;
esac
deb="rulec_$version-1_$arch.deb"
rpm="rulec-$version-1.$rpmarch.rpm"

nfpm="goreleaser/nfpm:v2.47.0@sha256:a662cb167d7b6d3a83920c83d76b12d02b8ac5dd2c13e5c62c15270b23f6df0c"
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$bin" "$stage/rulec"

pack() {
  docker run --rm --user "$(id -u):$(id -g)" \
    -v "$root:/src:ro" -v "$stage:/stage:ro" -v "$out:/out" -w /src \
    -e RULEC_ARCH="$arch" -e RULEC_VERSION="$version" -e RULEC_BINARY=/stage/rulec \
    "$nfpm" package --config packaging/nfpm.yaml --packager "$1" --target "/out/$2"
}
pack deb "$deb"
pack rpm "$rpm"

# Each package is installed where it belongs, with no network, so a dependency it should not
# have fails here. It has to put a working rulec on the path — one that checks the corpus
# clean, as release.yml asks of the binary itself — and has to take it away again when removed.
docker run --rm --network none --platform "linux/$arch" -v "$out:/out:ro" -v "$root/tests/corpus:/corpus:ro" \
  -e DEBIAN_FRONTEND=noninteractive debian:stable-slim sh -euc '
    apt-get install -y -qq "/out/$1" > /dev/null
    rulec --version
    rulec check /corpus/ > /dev/null
    apt-get remove -y -qq rulec > /dev/null
    test ! -e /usr/bin/rulec' sh "$deb"
docker run --rm --network none --platform "linux/$arch" -v "$out:/out:ro" -v "$root/tests/corpus:/corpus:ro" \
  fedora:latest sh -euc '
    dnf install -y -q --disablerepo="*" "/out/$1" > /dev/null
    rulec --version
    rulec check /corpus/ > /dev/null
    dnf remove -y -q rulec > /dev/null
    test ! -e /usr/bin/rulec' sh "$rpm"

ls "$out/$deb" "$out/$rpm"
