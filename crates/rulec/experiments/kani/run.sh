#!/bin/sh
# Every rule in the corpus, through the model checker: generate, then read the proof
# harnesses `gen` writes beside the Rust. Needs `kani` on the PATH (§15.95).
#
#   $ cargo build --release
#   $ sh experiments/kani/run.sh > experiments/kani/report.txt
set -eu
out=$(mktemp -d)
./target/release/rulec gen tests/corpus/ --out "$out" > /dev/null
echo "rulec $(./target/release/rulec --version | awk '{print $2}'), $(kani --version | tr '\n' ' ')"
echo
cd "$out/rust"
total=0
for f in *_proof.rs; do
  s=$(date +%s)
  log=$(kani "$f" 2>&1) || true
  e=$(date +%s)
  line=$(printf '%s\n' "$log" | grep -E '^Complete - ' | tail -1)
  # A rule the generator deliberately writes no harness for says so in the file itself
  # (a division by a value, §15.102); kani then reports that it found nothing to verify.
  # That is not a missing verdict, and reading it as one made the report look alarming.
  if [ -z "$line" ] && printf '%s\n' "$log" | grep -q 'No proof harnesses'; then
    line="no harness written — see the head of $f"
  fi
  printf '%-24s %3ds  %s\n' "${f%_proof.rs}" "$((e - s))" "${line:-NO VERDICT}"
  total=$((total + e - s))
done
echo
echo "総計 ${total}s"
