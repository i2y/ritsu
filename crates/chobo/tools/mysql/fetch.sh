#!/bin/sh
# Fetches MySQL 8.4.11, the server the tests of the MySQL family start (tests/backends.rs,
# tests/mysql.rs), from MySQL's downloads into tools/mysql/mysql (which .gitignore leaves out):
# the minimal build on Linux, the build for macOS 15 on a Mac. It checks the archive's SHA-256
# before it unpacks it, and prints the directory to give RITSU_MYSQL_BIN. Run it from anywhere:
#
#   sh tools/mysql/fetch.sh
set -eu

VERSION=8.4.11
here=$(cd "$(dirname "$0")" && pwd)

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    file=mysql-$VERSION-macos15-arm64.tar.gz
    want=b96e00493bc3499b9ffd7f08d65c5d64933af0383a8287d9873b64f94c2d6009 ;;
  Linux-x86_64)
    file=mysql-$VERSION-linux-glibc2.28-x86_64-minimal.tar.xz
    want=383f54e124d5f325d67f0c6912a8f96814eedc761a17ea30112e52fa4cc6b143 ;;
  *)
    echo "the tests fetch MySQL $VERSION for macOS on arm64 and Linux on x86_64, not $(uname -s) $(uname -m); set RITSU_MYSQL_BIN to a MySQL of your own" >&2
    exit 1 ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/$file" "https://cdn.mysql.com/Downloads/MySQL-8.4/$file"

if command -v sha256sum > /dev/null; then
  got=$(sha256sum "$tmp/$file" | cut -d ' ' -f 1)
else
  got=$(shasum -a 256 "$tmp/$file" | cut -d ' ' -f 1)
fi
if [ "$got" != "$want" ]; then
  echo "the SHA-256 of $file is $got, not $want" >&2
  exit 1
fi

mkdir "$tmp/unpacked"
tar -xf "$tmp/$file" -C "$tmp/unpacked"
rm -rf "$here/mysql"
mv "$tmp/unpacked"/mysql-* "$here/mysql"
# what the tests do not run: the debug server, and the manual
rm -rf "$here/mysql/bin/mysqld-debug" "$here/mysql/lib/plugin/debug" "$here/mysql/docs" "$here/mysql/man"
"$here/mysql/bin/mysqld" --version
echo "$here/mysql/bin"
