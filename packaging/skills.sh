#!/bin/sh
# Writes the archive of the Agent Skills of one release: every folder of skills/ at the root of the
# repository (ritsu's and one for each of the seven languages), each as it is, and the two licenses
# (DESIGN 13.2). release.yml runs it once for a release, and crates/ritsu/tests/skill.rs runs it on
# the repository.
#
#   sh packaging/skills.sh v0.23.0 dist/
#
# The folders sit at the top of the archive, so unzipping it into the directory an agent reads skills
# from is the whole install: `unzip ritsu-skills-v0.23.0.zip -d ~/.claude/skills -x 'LICENSE-*'`
# (without `-x`, LICENSE-MIT and LICENSE-APACHE land there too). They are the files `ritsu skills
# install` writes, which the binary of the same version carries.
set -eu

tag=$1
out=$2

# The skills and the licenses sit at the root of the repository, one up from this script.
root=$(cd "$(dirname "$0")/.." && pwd)
licenses="LICENSE-MIT LICENSE-APACHE"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
skills=""
for d in "$root"/skills/*/; do
  name=$(basename "$d")
  test -f "$d/SKILL.md" || { echo "skills/$name has no SKILL.md" >&2; exit 1; }
  cp -R "$d" "$stage/$name"
  skills="$skills $name"
done
test -n "$skills" || { echo "skills/ holds no skill" >&2; exit 1; }
for f in $licenses; do
  cp "$root/$f" "$stage/$f"
done

mkdir -p "$out"
zip=$(cd "$out" && pwd)/ritsu-skills-$tag.zip
rm -f "$zip"
# -X leaves out the owners and the extra times of the files, so the archive holds the files alone.
# shellcheck disable=SC2086
(cd "$stage" && zip -q -r -X "$zip" $skills $licenses)
echo "$out/ritsu-skills-$tag.zip"
