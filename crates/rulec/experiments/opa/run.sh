#!/bin/sh
# Measure whether Rego can hold a rule's arithmetic without losing anything.
#
#   sh run.sh                     writes report.txt
#   OPA=./opa sh run.sh           a binary somewhere else
#
# Needs `opa` on the PATH (one static binary, no network at run time:
# https://github.com/open-policy-agent/opa/releases) and a built rulec.
set -e
cd "$(dirname "$0")"
RULEC=${RULEC:-../../target/release/rulec}
CORPUS=${CORPUS:-../../tests/corpus}
OPA=${OPA:-opa}
OUT=${OUT:-out}

command -v "$OPA" >/dev/null 2>&1 || { echo "opa がありません（OPA=... で場所を指定できます）" >&2; exit 1; }
[ -x "$RULEC" ] || { echo "rulec がありません: $RULEC（cargo build --release）" >&2; exit 1; }

rm -rf "$OUT" && mkdir -p "$OUT"
"$RULEC" gen "$CORPUS/送料.rule"  --out "$OUT/gen" >/dev/null
"$RULEC" gen "$CORPUS/ec261.rule" --out "$OUT/gen" >/dev/null
set -- $(python3 cases.py "$OUT/gen" "$OUT")
n_cases=$1 n_stress=$2 n_max=$3 n_vec=$4

# One value, printed compactly: `-f values` wraps every answer in a list of its own.
ev() {
  "$OPA" eval -d round.rego -d ec261.rego -f values "$@" |
    python3 -c 'import json,sys; t=sys.stdin.read().strip(); v=json.loads(t) if t else []; print(json.dumps(v[0] if len(v)==1 else v, ensure_ascii=False) if t else "undefined")'
}

{
  echo "rulec $("$RULEC" --version | awk '{print $NF}')"
  "$OPA" version | sed -n '1p;7p' | tr '\n' ' ' | sed 's/  */ /g'
  echo
  echo "date $(date +%Y-%m-%d)"
  echo

  echo "1. integers"
  echo "   int64 max round trip      $(ev '9223372036854775807')"
  echo "   123456789 * 987654321     $(ev '123456789 * 987654321')   exact: 121932631112635269"
  echo "   floor((2^53+1) / 1)       $(ev 'floor(9007199254740993 / 1)')   float64 would give 9007199254740992"
  echo "   floor(int64max/10)*10     $(ev 'floor(9223372036854775807 / 10) * 10')"
  echo "   int64max % 10             $(ev '9223372036854775807 % 10')"
  echo

  echo "2. the five modes, over the vectors rulec generates for them"
  echo "   cases        $n_cases"
  echo "   answered     $(ev -i "$OUT/cases.json" 'count(data.rulec.round.evaluated)')"
  echo "   mismatches   $(ev -i "$OUT/cases.json" 'count(data.rulec.round.failures)')"
  echo

  echo "3. the five modes, stressed to int64 (|x| up to $n_max, grids 1..1000000007)"
  echo "   cases        $n_stress"
  echo "   answered     $(ev -i "$OUT/stress.json" 'count(data.rulec.round.evaluated)')"
  echo "   mismatches   $(ev -i "$OUT/stress.json" 'count(data.rulec.round.failures)')"
  echo

  echo "4. one whole rule (ec261), over the vectors rulec generates for it"
  echo "   vectors      $n_vec"
  echo "   answered     $(ev -i "$OUT/ec261_vectors.json" 'count(data.rulec.ec261.answered)')"
  echo "   mismatches   $(ev -i "$OUT/ec261_vectors.json" 'data.rulec.ec261.failures')"
  echo "   guard        $(ev 'data.rulec.ec261.result(0,true,0)')"
  echo "                $(ev 'data.rulec.ec261.result(1500,true,3)')"
  echo

  echo "5. the division transcribed literally"
  echo "   (7/2)*2                   $(ev '(7 / 2) * 2')   truncating would give 6"
  echo "   naive vs the $n_vec vectors   $(ev -i "$OUT/ec261_vectors.json" 'count([1 | some v in input.vectors; data.rulec.ec261.naive(v.in.distance, v.in.intra_eu, v.in.delay) != v.out.compensation])') mismatches — the rule's own vectors do not catch it"
  echo "   scale 100, value 12345    $(ev '{"correct": data.rulec.round.apply("down", 12345, 100) / 100, "naive": ((12345 / 100) * 100) / 100}')"
  echo

  echo "6. a value no row names"
  echo "   _base(\"xl\")               $(ev 'data.rulec.ec261._base("xl")')   — no value at all, not an error"
} | tee report.txt
