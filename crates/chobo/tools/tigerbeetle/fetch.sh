#!/bin/sh
# Fetches TigerBeetle 0.17.9, the release chobo's clients and tests are pinned to, from GitHub's
# releases into tools/tigerbeetle/tigerbeetle (which .gitignore leaves out). It checks the zip's
# SHA-256 before it unpacks it. Run it from anywhere:
#
#   sh tools/tigerbeetle/fetch.sh
set -eu

VERSION=0.17.9
here=$(cd "$(dirname "$0")" && pwd)

case "$(uname -s)-$(uname -m)" in
  Darwin-*)
    zip=tigerbeetle-universal-macos.zip
    want=4e085eaffc66c2ed82e7f94a8137c468256d2fb0b35152ccf903cc6676c22940 ;;
  Linux-x86_64)
    zip=tigerbeetle-x86_64-linux.zip
    want=af71f2c0057e3b409bf79940fa94894b738187b0d4f1e711097bca15df5d8cd4 ;;
  Linux-aarch64 | Linux-arm64)
    zip=tigerbeetle-aarch64-linux.zip
    want=413994920fe48b04f5aa86895b7a1d9d98d14803b29d4e91d2a6d0098fff4ef2 ;;
  *)
    echo "TigerBeetle $VERSION has no release for $(uname -s) $(uname -m)" >&2
    exit 1 ;;
esac

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/$zip" "https://github.com/tigerbeetle/tigerbeetle/releases/download/$VERSION/$zip"

if command -v sha256sum > /dev/null; then
  got=$(sha256sum "$tmp/$zip" | cut -d ' ' -f 1)
else
  got=$(shasum -a 256 "$tmp/$zip" | cut -d ' ' -f 1)
fi
if [ "$got" != "$want" ]; then
  echo "the SHA-256 of $zip is $got, not $want" >&2
  exit 1
fi

unzip -q "$tmp/$zip" -d "$tmp/unpacked"
mv "$tmp/unpacked/tigerbeetle" "$here/tigerbeetle"
chmod +x "$here/tigerbeetle"
"$here/tigerbeetle" version
