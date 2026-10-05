#!/bin/sh
# Builds the .deb and the .rpm of one release from its static Linux binary, and installs each on
# the distribution it is for before either is kept (DESIGN 13.2): a package that does not install
# is worse than none. release.yml runs it on both Linux runners, and packages.yml on every push,
# so a release is not the first time it runs.
#
#   sh packaging/linux.sh 0.23.0 x86_64-unknown-linux-musl target/x86_64-unknown-linux-musl/release/ritsu dist/
#
# It needs docker and nothing else. nfpm writes the packages from packaging/nfpm.yaml and runs from
# its image, pinned by digest as CI pins its other tools. Each package has to hold the two licenses
# and THIRD_PARTY_NOTICES in /usr/share/doc/ritsu, as its own list of files says (the slim Debian
# image leaves /usr/share/doc out when it installs), and is installed with no network, run under
# all seven names (packaging/smoke.sh), and removed, and what it put in /usr/bin has to go with it.
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
deb="ritsu_$version-1_$arch.deb"
rpm="ritsu-$version-1.$rpmarch.rpm"

nfpm="goreleaser/nfpm:v2.47.0@sha256:a662cb167d7b6d3a83920c83d76b12d02b8ac5dd2c13e5c62c15270b23f6df0c"
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$out"
out=$(cd "$out" && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp "$bin" "$stage/ritsu"

pack() {
  docker run --rm --user "$(id -u):$(id -g)" \
    -v "$root:/src:ro" -v "$stage:/stage:ro" -v "$out:/out" -w /src \
    -e RITSU_ARCH="$arch" -e RITSU_VERSION="$version" -e RITSU_BINARY=/stage/ritsu \
    "$nfpm" package --config packaging/nfpm.yaml --packager "$1" --target "/out/$2"
}
pack deb "$deb"
pack rpm "$rpm"

# Each package is installed where it belongs, with no network, so a dependency it should not have
# fails here. It has to put a working ritsu on the path under all seven names, which read the
# examples as release.yml asks of the archive, and has to take all eight away again when removed.
docker run --rm --network none --platform "linux/$arch" -v "$out:/out:ro" -v "$root/crates:/crates:ro" -v "$root/packaging:/packaging:ro" \
  -e DEBIAN_FRONTEND=noninteractive debian:stable-slim sh -euc '
    for f in LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES; do
      dpkg-deb -c "/out/$1" | grep -q " ./usr/share/doc/ritsu/$f\$"
    done
    apt-get install -y -qq "/out/$1" > /dev/null
    ritsu --version
    sh /packaging/smoke.sh - /crates
    apt-get remove -y -qq ritsu > /dev/null
    for n in ritsu rulec dandori koyomi chobo geas yuen sakai; do
      test ! -e "/usr/bin/$n" && test ! -L "/usr/bin/$n"
    done' sh "$deb"
docker run --rm --network none --platform "linux/$arch" -v "$out:/out:ro" -v "$root/crates:/crates:ro" -v "$root/packaging:/packaging:ro" \
  fedora:latest sh -euc '
    for f in LICENSE-MIT LICENSE-APACHE THIRD_PARTY_NOTICES; do
      rpm -qlp "/out/$1" | grep -qx "/usr/share/doc/ritsu/$f"
    done
    dnf install -y -q --disablerepo="*" "/out/$1" > /dev/null
    ritsu --version
    sh /packaging/smoke.sh - /crates
    dnf remove -y -q ritsu > /dev/null
    for n in ritsu rulec dandori koyomi chobo geas yuen sakai; do
      test ! -e "/usr/bin/$n" && test ! -L "/usr/bin/$n"
    done' sh "$rpm"

ls "$out/$deb" "$out/$rpm"
