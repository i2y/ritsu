#!/bin/sh
# Ask every condition for every model once (answers are kept, re-runs skip what exists),
# then compare each answer file against the rule with `rulec verify` under two policies.
#   sh run_all.sh [models...]      default: claude-opus-5 claude-sonnet-5
#   BACKEND=ollama EFFORT=nothink sh run_all.sh qwen3.6:35b-a3b     a local model instead
# The SDK backend needs ANTHROPIC_API_KEY (or an `ant auth login` profile); the Ollama
# backend needs the server on localhost:11434 and takes `think` or `nothink` as EFFORT.
set -e
cd "$(dirname "$0")"
RULEC=${RULEC:-../../target/release/rulec}
CORPUS=${CORPUS:-../../tests/corpus}
PY=${PY:-$([ -x .venv/bin/python ] && echo .venv/bin/python || echo python3)}
K=${K:-3}
CONC=${CONC:-6}
EFFORT=${EFFORT:-low}
BACKEND=${BACKEND:-sdk}
MODELS=${*:-"claude-opus-5 claude-sonnet-5"}
mkdir -p runs
for m in $MODELS; do
  mt=$(printf '%s' "$m" | tr '/:' '__')   # a file name for the model
  for cond in none page rule doc article; do
    ctx=ctx/ゆうパック運賃.$cond.txt
    case $cond in none) ctx=none;; rule) ctx=ctx/ゆうパック運賃.rule;; doc) ctx=ctx/ゆうパック運賃.doc.md;; esac
    [ "$ctx" = none ] || [ -f "$ctx" ] || { echo "skip yupack/$cond: no $ctx"; continue; }
    $PY ask.py --rule ゆうパック運賃 --vectors ctx/ゆうパック運賃.vectors.jsonl --context "$ctx" --label "$cond" \
      --model "$m" --backend "$BACKEND" --effort "$EFFORT" --samples "$K" --concurrency "$CONC" --out "runs/yupack.$cond.$mt.$EFFORT.jsonl"
  done
  for cond in rule doc article; do
    ctx=ctx/会員特典.$cond.txt
    case $cond in rule) ctx=ctx/会員特典.rule;; doc) ctx=ctx/会員特典.doc.md;; esac
    [ -f "$ctx" ] || { echo "skip perk/$cond: no $ctx"; continue; }
    $PY ask.py --rule 会員特典 --vectors ctx/会員特典.vectors.jsonl --context "$ctx" --label "$cond" \
      --model "$m" --backend "$BACKEND" --effort "$EFFORT" --samples "$K" --concurrency "$CONC" --out "runs/perk.$cond.$mt.$EFFORT.jsonl"
  done
done
for f in runs/yupack.*.jsonl runs/perk.*.jsonl; do
  [ -f "$f" ] || continue
  case $f in *.verify.*) continue;; esac
  case $f in runs/yupack.*) rule=ゆうパック運賃;; *) rule=会員特典;; esac
  for policy in first all-agree; do
    base=${f%.jsonl}
    $RULEC verify "$CORPUS/$rule.rule" --lang ja --adapter $PY adapter.py --answers "$f" --policy $policy > "$base.verify.$policy.txt" 2>&1 || true
    $RULEC verify "$CORPUS/$rule.rule" --format json --adapter $PY adapter.py --answers "$f" --policy $policy > "$base.verify.$policy.json" 2>/dev/null || true
  done
done
$PY report.py runs/*.jsonl
