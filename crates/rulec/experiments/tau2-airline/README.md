# τ²-bench's airline policy, with its tables as tools

Three of the airline policy's rules are tables: the free checked-bag allowance by membership
and cabin, whether a reservation can be cancelled, and the compensation certificate a
complaint earns. They are transcribed in `tests/corpus/預け荷物料金.rule`, `予約取消可否.rule`
and `補償証明書.rule`, and `tau2_rulec/` adds the Python `rulec gen` writes from them to the
airline toolkit as three tools (`baggage_fee`, `cancellation_verdict`, `compensation_amount`),
each returning its answer with the table rows that decided it. The policy gets one added
section that names the tools (`tau2_rulec/policy_rulec.md`); the database and the tasks are
the airline domain's own.

What transcribing found: the policy tells the agent to collect the reason for cancelling as
one of three (change of plan, airline cancelled, other), and says elsewhere that insurance
covers health or weather reasons. A table needs health and weather in the reason column, and
whether the airline cancelled is a flight's status, not the user's word. Both are written in
the rules' descriptions.

```console
$ sh setup.sh                      # clone, install, generate, register; runs smoke.py
$ cd tau2-bench
$ .venv/bin/tau2 run --domain airline --task-set-name airline --agent-llm anthropic/claude-sonnet-5 --user-llm anthropic/claude-sonnet-5 --num-trials 3 --save-to without_tools
$ .venv/bin/tau2 run --domain airline_rulec --task-set-name airline --agent-llm anthropic/claude-sonnet-5 --user-llm anthropic/claude-sonnet-5 --num-trials 3 --save-to with_tools
```

The two runs differ only in the domain; `compare.sh <model> [trials] [task ids]` runs both
and `summarize.py` prints the rewards, pass^k and the tasks that differ. τ²-bench reaches
models through LiteLLM, so a key goes in `tau2-bench/.env` (`ANTHROPIC_API_KEY=…`), and a
local model needs none (`ollama_chat/qwen3.6:35b-a3b`). Of the 50 airline tasks, 15 mention bags,
30 cancellation and 8 a certificate (counted over the task text), so those are where the two
runs can differ.

A reasoning model through Ollama must have its thinking switched off (`compare.sh` passes
`{"think": false}` to both roles): with it on, qwen3.6 sometimes puts the whole answer into
its thinking and returns an empty message, which τ²-bench refuses as an infrastructure error.
With it off, one airline task takes about a minute on an M4 Max.

## Results so far (2026-09-19, qwen3.6:35b-a3b through Ollama, thinking off)

Fifteen tasks that touch cancellation or compensation (0 1 4 5 26 27 28 38 39 41 43 45 47 48
49), one trial each:

| run | avg reward |
|---|---|
| without tools | 0.800 |
| with tools | 0.867 |

Tasks 43 and 47 turn from 0 to 1 with the tools: without them the agent cancelled a
reservation the policy does not allow to cancel; with them it called
`cancellation_verdict`, got `refused`, and did not. Task 41 turns from 1 to 0 without any
tool being called: the agent loops on reservation look-ups until the step limit, which task
39 does in both runs. `cancellation_verdict` was called in six tasks; the other two tools had
no occasion here. One trial cannot separate the effect from the noise; the same comparison with three
trials is `sh compare.sh ollama_chat/qwen3.6:35b-a3b 3 0 1 4 5 26 27 28 38 39 41 43 45 47 48 49`,
about two and a half hours on an M4 Max.
