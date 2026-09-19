#!/usr/bin/env python3
"""report.py: one table over every answers file, plus where the wrong answers sit.

Per file: vectors, per-sample accuracy, how many vectors every sample got right, how many
every sample got wrong the same way (a stable miss), how many samples disagreed, how many
gave no readable answer, and the token spend. Then, for each file, the wrong answers grouped
by the rows that fired, so a miss that sits on one boundary shows as one line.
"""
import collections, json, sys

PRICE = {"claude-opus-5": (5.0, 25.0), "claude-sonnet-5": (2.0, 10.0), "claude-haiku-4-5": (1.0, 5.0)}


def canon(d):
    return json.dumps(d, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def load(path):
    meta, recs = None, []
    for l in open(path, encoding="utf-8"):
        if not l.strip():
            continue
        d = json.loads(l)
        if "meta" in d:
            meta = d["meta"]
        else:
            recs.append(d)
    return meta, recs


rows, details = [], []
by_kind, by_fair = {}, {}
for path in sys.argv[1:]:
    meta, recs = load(path)
    if not recs:
        continue
    k = max(len(r["samples"]) for r in recs)
    per = [0] * k
    all_ok = stable_wrong = disagree = unanswered = 0
    usage = collections.Counter()
    wrong_by_rows = collections.defaultdict(list)
    for r in recs:
        exp = canon(r["out"])
        parsed = [s.get("parsed") for s in r["samples"]]
        for i, p in enumerate(parsed):
            if p is not None and canon(p) == exp:
                per[i] += 1
        got = {canon(p) if p is not None else None for p in parsed}
        if None in got:
            unanswered += 1
        elif len(got) > 1:
            disagree += 1
        elif exp in got:
            all_ok += 1
        else:
            stable_wrong += 1
        for i, p in enumerate(parsed):
            if p is None or canon(p) != exp:
                wrong_by_rows[" / ".join(r.get("trace", []))].append((r["in"], r["out"], p, i))
        for s in r["samples"]:
            for key, v in (s.get("usage") or {}).items():
                usage[key] += v or 0
    n = len(recs)
    pin, pout = PRICE.get(meta["model"], (0, 0))
    cost = (usage["input"] * pin + usage["cache_write"] * pin * 1.25 + usage["cache_read"] * pin * 0.1 + usage["output"] * pout) / 1e6
    rows.append((meta["rule"], meta["label"], meta["model"], meta.get("effort"), n,
                 " ".join(f"{c * 100 / n:.1f}%" for c in per), all_ok, stable_wrong, disagree, unanswered, f"${cost:.2f}"))
    # Accuracy by the kind of vector (first sample only). The boundary pairs are quoted
    # verbatim in the customer article, so an article condition is fair only on the rest.
    kinds = collections.OrderedDict()
    for r in recs:
        kind = r.get("why", "").split(":")[0].split("（")[0].strip()
        kind = "example" if kind.startswith("example") or kind.startswith("例") else kind
        ok = r["samples"][0].get("parsed") is not None and canon(r["samples"][0]["parsed"]) == canon(r["out"])
        kinds.setdefault(kind, [0, 0])
        kinds[kind][0] += ok
        kinds[kind][1] += 1
    by_kind[(meta["rule"], meta["label"], meta["model"])] = kinds
    # The vectors the customer article does not quote: pairwise and row targets. This is the
    # column to compare conditions on.
    quoted = ("boundary", "example", "baseline")
    fair = [(k, v) for k, v in kinds.items() if not k.startswith(quoted)]
    fair_ok, fair_n = sum(v[0] for _, v in fair), sum(v[1] for _, v in fair)
    by_fair[(meta["rule"], meta["label"], meta["model"])] = (fair_ok, fair_n)
    details.append((path, meta, n, wrong_by_rows))

print("| rule | context | model | effort | n | correct per sample | all right | stable miss | unstable | no answer | est. cost |")
print("|---|---|---|---|---|---|---|---|---|---|---|")
for r in rows:
    print("| " + " | ".join(str(x) for x in r) + " |")
allk = sorted({k for ks in by_kind.values() for k in ks})
print("\n| rule | context | model | " + " | ".join(allk) + " | not quoted (pairwise + row target) |")
print("|---|---|---|" + "---|" * (len(allk) + 1))
for (rule, label, model), ks in by_kind.items():
    cells = [f"{ks[k][0]}/{ks[k][1]}" if k in ks else "" for k in allk]
    fo, fn = by_fair[(rule, label, model)]
    pct = f"{fo * 100 / fn:.1f}%" if fn else ""
    print(f"| {rule} | {label} | {model} | " + " | ".join(cells) + f" | {fo}/{fn} = {pct} |")
for path, meta, n, wrong in details:
    if not wrong:
        continue
    print(f"\n### {meta['rule']} / {meta['label']} / {meta['model']} — wrong answers by the rows that fired")
    for rows_key, items in sorted(wrong.items(), key=lambda kv: -len(kv[1])):
        i0 = items[0]
        print(f"- {len(items):3d}  {rows_key}   e.g. {canon(i0[0])} → rule {canon(i0[1])} / model {canon(i0[2]) if i0[2] else '-'}")
