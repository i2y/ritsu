# A model as the counterpart of `rulec verify`

The adapter protocol of `rulec verify` does not care what answers on the other end. Here the
other end is a model reading a document, asked the rule's own boundary vectors one by one —
the setting of a support bot that answers "how much is the fare" from a help page.

- `ask.py` puts every vector of a rule to a model, k times, under one condition (which
  document is in the context), and keeps every answer in `runs/*.jsonl`.
- `adapter.py` answers `rulec verify` from that file, so the model is asked once and the
  comparison can be re-run under any policy (`all-agree`, `majority`, `first`).
- `report.py` is one table over every answers file, plus the wrong answers grouped by the
  rows that fired and the accuracy by kind of vector.
- `prepare.sh` writes the contexts; `run_all.sh` runs every condition for every model, then
  `rulec verify` on each.

Conditions, for ゆうパック運賃: `none` (no document), `page` (the text of 日本郵便's fare
page), `rule` (the `.rule` file), `doc` (`rulec doc`), `article` (`rulec doc --audience
customer`). The boundary pairs are quoted verbatim in the article, so read the article
condition on the other kinds of vector (`report.py` splits them out).

```console
$ uv venv .venv && uv pip install --python .venv/bin/python anthropic
$ sh prepare.sh
$ ANTHROPIC_API_KEY=… sh run_all.sh claude-opus-5 claude-sonnet-5
```

`runs/pilot.*.jsonl` are the pilot runs of 2026-09-19: the boundary vectors only, one sample,
through the Claude Code CLI (`--backend cli`), which adds its own system prompt and allows no
sampling control — enough to see the shape, not the numbers.

## Results so far (2026-09-19, local models through Ollama)

Every condition, both rules, k = 3, temperature 0.7, thinking off. The column is the match
rate on the vectors the customer article does not quote (pairwise and row targets: 187 for
ゆうパック運賃, 80 for 会員特典); `report.py` prints the rest.

| context | ゆうパック qwen3.6:35b-a3b | ゆうパック Swallow-8B | 会員特典 qwen3.6 | 会員特典 Swallow-8B |
|---|---|---|---|---|
| none | 0.5% | 0.0% | — | — |
| page | 67.9% | 32.1% | — | — |
| rule | 71.1% | 44.4% | 22.5% | 6.2% |
| doc | 67.4% | 27.8% | 25.0% | 12.5% |
| article | 79.1% | 58.8% | 38.8% | 22.5% |

The misses sit on the outside of the size boundaries and on the region rows (東京都 read
off the 近距離圏 row); `rulec verify` clusters them that way in `runs/*.verify.first.txt`.
The composed rule with rounding (会員特典) collapses under every condition. Frontier models
through the API are still to be run; `run_all.sh` takes them as is.
