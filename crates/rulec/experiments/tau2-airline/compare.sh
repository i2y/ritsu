#!/bin/sh
# Run the same airline tasks twice — the domain as it is, and with the rulec-generated tools —
# and summarise the two. τ²-bench reaches models through LiteLLM, so a model name is a
# LiteLLM one: `anthropic/claude-sonnet-5`, `ollama_chat/qwen3.6:35b-a3b`, ...
#   sh compare.sh <model> [num_trials] [task ids...]
#   MODEL=ollama_chat/qwen3.6:35b-a3b sh compare.sh $MODEL 1 0 1 2
# Results land in tau2-bench/data/simulations/{without,with}_tools_<tag>_t<trials>/results.json and
# summarize.py prints the comparison.
set -e
cd "$(dirname "$0")"
MODEL=${1:?model, e.g. ollama_chat/qwen3.6:35b-a3b}
TRIALS=${2:-1}
shift; [ $# -gt 0 ] && shift
TAG=$(printf '%s' "$MODEL" | tr '/:.' '___')
# A reasoning model answers with an empty message when its whole answer went into thinking,
# which τ²-bench refuses; through Ollama, thinking is switched off unless LLM_ARGS says otherwise.
case $MODEL in ollama*) LLM_ARGS=${LLM_ARGS:-'{"think": false}'};; *) LLM_ARGS=${LLM_ARGS:-'{}'};; esac
TASKS=""
[ $# -gt 0 ] && TASKS="--task-ids $*"
cd tau2-bench
for d in airline airline_rulec; do
  case $d in airline) name=without_tools_${TAG}_t$TRIALS;; *) name=with_tools_${TAG}_t$TRIALS;; esac
  .venv/bin/tau2 run --domain $d --task-set-name airline --agent-llm "$MODEL" --user-llm "$MODEL" \
    --agent-llm-args "$LLM_ARGS" --user-llm-args "$LLM_ARGS" \
    --num-trials "$TRIALS" --max-concurrency "${CONC:-1}" --max-steps "${MAX_STEPS:-60}" $TASKS --save-to "$name" \
    2>&1 | grep -vE "DEBUG|^\s+\"|^\s*[][{}],?$" | tail -5
done
cd ..
python3 summarize.py "tau2-bench/data/simulations/without_tools_${TAG}_t$TRIALS/results.json" "tau2-bench/data/simulations/with_tools_${TAG}_t$TRIALS/results.json"
