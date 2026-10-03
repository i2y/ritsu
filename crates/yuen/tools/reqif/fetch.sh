#!/bin/sh
# Fetch the schema of ReqIF 1.2 (OMG) and the W3C schemas it imports — 24 files — check
# each against the SHA-256 recorded when yuen was designed, and write an XML catalog that
# points their URLs at the files fetched, so that xmllint validates without the network:
#
#   XML_CATALOG_FILES=tools/reqif/xsd/catalog.xml xmllint --nonet --noout \
#     --schema tools/reqif/xsd/www.omg.org/spec/ReqIF/20110401/reqif.xsd out.reqif
#
# Usage: tools/reqif/fetch.sh [<dir>]
# The files go to <dir> (default: xsd/ beside this script), each at <host>/<path> of its
# URL. They belong to the OMG and the W3C and are not kept in the repository; see README.md.

set -eu

here=$(cd "$(dirname "$0")" && pwd)
dest=${1:-$here/xsd}

if command -v shasum >/dev/null 2>&1; then
  sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
elif command -v sha256sum >/dev/null 2>&1; then
  sha() { sha256sum "$1" | cut -d' ' -f1; }
else
  echo "fetch.sh: neither shasum nor sha256sum is on the PATH" >&2
  exit 2
fi

m=www.w3.org/TR/xhtml-modularization/SCHEMA
files="9243f345540f25db3b53403da9ad9cd4744277ef01492ac3589937f533ba94c0 www.omg.org/spec/ReqIF/20110401/reqif.xsd
4995bc97cf0a9b8462ca295006dd54d9a85fb820cf9fd6e134a51743fc44effd www.omg.org/spec/ReqIF/20110402/driver.xsd
61960fb3131e38022caad5360e2f33a3382578ab3c80cd58bd74320ede61b20c www.w3.org/2001/xml.xsd
cc701736c42cc64126fad063bb95f94484b5de3b5f808a86ea098b0957aff829 www.w3.org/2009/01/xml.xsd
ab0c593a06a60a5fee2b77ec9283394a3974079b75589ced177944e627f4083e $m/xhtml-attribs-1.xsd
34479ecd862fea5ed1d8eb2c561bc5fcb75ee0443046aead242a123b5cff49e3 $m/xhtml-blkphras-1.xsd
4f40e2d55ea57a7356d638d6b562f828e33fc7804002cf26823259f3f5e95e67 $m/xhtml-blkpres-1.xsd
080afe0ce1da906020e53d9385151790416a8255f5a1f6b1757c02ecb8edecfa $m/xhtml-blkstruct-1.xsd
0559368d5dba054a941537296a77ae327209c172dd1facad3ae05bab09a0a9ae $m/xhtml-charent-1.xsd
cb5a32da43a65d91cf7ee6a42cc47a8ff70796256587884b7bac9d1444938fd7 $m/xhtml-datatypes-1.xsd
d7b2af85393f31c8a0325ffbfea351e80929af0824e3f2eaf26e2b34d1c0196d $m/xhtml-edit-1.xsd
425afbe531545f084783a077b2dcb44b09f23ba36536c3547a70ef100a6dd02d $m/xhtml-framework-1.xsd
88344fdf2a127ae4449d7b6c813f8b755ca6eb71c8583a50f6012e2459cf7e30 $m/xhtml-hypertext-1.xsd
3e5234930694552e6f69f4c133ba57db24944e4bb346169ad5718182e8aec07c $m/xhtml-inlphras-1.xsd
0ee7157cf3f99900d15a3a1a4b8daf50876623631e3cab145710b0eda6423541 $m/xhtml-inlpres-1.xsd
ec861146ae81ad25331130865513951aef1852f2952f7b4604ca3456f9da97cc $m/xhtml-inlstruct-1.xsd
764b9e6edf496f348449727610d7d19ec24b1027bacc352636200db7976a06c5 $m/xhtml-inlstyle-1.xsd
74bd94dcdbc889e2006fa354c27b85b3ee0e8e06069827c4db796c060b96b4a1 $m/xhtml-list-1.xsd
c623c60fb9cd2c26cb8aaddc8df5d7dabe99e573e354437ac44e12d9c7e6cdef $m/xhtml-notations-1.xsd
4862dd0a3ab00eb5cd5c29a46736f89461b822f074d743d3faa3d6c8a200cbdb $m/xhtml-object-1.xsd
99d1df8882159a1ec923b064450e207ce03443b621045d438faa654e402820e3 $m/xhtml-param-1.xsd
1bcde28585c9372b8cdbb2dc8c44b4f04e798446df415d6f6e494ce1e531791e $m/xhtml-pres-1.xsd
11b510f8a116b937b20b13339fbf33bbc9bc133e917ee3df4f3b5abba3f63fc2 $m/xhtml-table-1.xsd
527da2d8384ea77159648dc85e81703d402b9dbe5ae099816c61ff9994a3ca5d $m/xhtml-text-1.xsd"

mkdir -p "$dest"
catalog="$dest/catalog.xml.tmp"
{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo '<!-- Written by tools/reqif/fetch.sh: the URLs of the ReqIF schema and what it imports, pointed at the files beside this catalog. -->'
  echo '<catalog xmlns="urn:oasis:names:tc:entity:xmlns:xml:catalog">'
} > "$catalog"

echo "$files" | while read -r want path; do
  file="$dest/$path"
  mkdir -p "$(dirname "$file")"
  if [ ! -f "$file" ] || [ "$(sha "$file")" != "$want" ]; then
    # The W3C and the OMG serve these over https; the schemas name them by their http URLs.
    curl -fsSL --retry 2 --max-time 60 -o "$file.tmp" "https://$path"
    got=$(sha "$file.tmp")
    if [ "$got" != "$want" ]; then
      rm -f "$file.tmp"
      echo "fetch.sh: https://$path has SHA-256 $got, not the $want recorded in this script" >&2
      exit 1
    fi
    mv "$file.tmp" "$file"
  fi
  echo "  <uri name=\"http://$path\" uri=\"$path\"/>" >> "$catalog"
  echo "  <system systemId=\"http://$path\" uri=\"$path\"/>" >> "$catalog"
done

echo '</catalog>' >> "$catalog"
mv "$catalog" "$dest/catalog.xml"
echo "fetch.sh: 24 schema files and catalog.xml in $dest"
