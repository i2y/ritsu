#!/bin/sh
# Bring the seven repositories into this workspace, each with its whole history, under
# crates/<name>/ (DESIGN 12.2):
#
#   tools/import.sh <work directory> [<directory that holds the seven repositories>]
#
# The second argument defaults to $HOME. Each repository is cloned into <work>/import/<name> and
# only the clone is rewritten: git filter-repo moves the files of every commit under
# crates/<name>/ and renames the tags to <name>/<tag>, keeping the authors and the dates. The
# original repositories are only read. Each clone is then merged into the repository this
# script sits in, which makes one merge commit per language at the time it runs.
set -eu

work=${1:?usage: tools/import.sh <work directory> [<directory that holds the seven repositories>]}
from=${2:-$HOME}
dest=$(cd "$(dirname "$0")/.." && pwd)

if [ -n "$(git -C "$dest" status --porcelain)" ]; then
  echo "import.sh: $dest has changes that are not committed" >&2
  exit 1
fi

for name in rulec dandori koyomi chobo geas yurai sakai; do
  clone=$work/import/$name
  rm -rf "$clone"
  git clone --quiet --no-local "$from/$name" "$clone"
  git -C "$clone" filter-repo --quiet --to-subdirectory-filter "crates/$name" --tag-rename ":$name/"

  body="Every commit of $name comes along with its author and date, and its
files sit under crates/$name/ in each of them, so git log and git
blame on a path reach back past this merge."
  if [ -n "$(git -C "$clone" tag)" ]; then
    body="$body Its tags are renamed to
$name/<tag>, since rulec and dandori both have a v0.1.0."
  fi

  git -C "$dest" remote add "$name" "$clone"
  git -C "$dest" fetch --quiet "$name" --tags
  git -C "$dest" merge --quiet --allow-unrelated-histories --no-edit \
    -m "chore: bring $name into the workspace with its history" \
    -m "$body" \
    -m "Co-Authored-By: Claude <noreply@anthropic.com>" \
    "$name/main"
  git -C "$dest" remote remove "$name"
  echo "import.sh: $name is in crates/$name/"
done
