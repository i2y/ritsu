#!/usr/bin/env python3
"""adapter.py: answer `rulec verify` from a file ask.py wrote.

  rulec verify rules/ゆうパック運賃.rule --adapter python3 adapter.py --answers runs/x.jsonl --policy all-agree

Policies: all-agree (every sample gave the same answer, else unanswered), majority, first,
sample:<i>. An unanswered record is excluded from verify's denominator and counted apart.
"""
import argparse, collections, json, sys


def canon(d):
    return json.dumps(d, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


ap = argparse.ArgumentParser()
ap.add_argument("--answers", required=True)
ap.add_argument("--policy", default="all-agree")
ap.add_argument("--impl", default=None)
a = ap.parse_args()

table, meta = {}, None
for line in open(a.answers, encoding="utf-8"):
    if not line.strip():
        continue
    d = json.loads(line)
    if "meta" in d:
        meta = d["meta"]
    else:
        table[d["key"]] = d


def decide(rec):
    parsed = [s.get("parsed") for s in rec["samples"]]
    if a.policy == "first":
        parsed = parsed[:1]
    elif a.policy.startswith("sample:"):
        parsed = parsed[int(a.policy.split(":")[1]):][:1]
    if a.policy == "majority":
        c = collections.Counter(canon(p) for p in parsed if p is not None)
        if not c:
            return None, "no sample answered"
        top, n = c.most_common(1)[0]
        if n * 2 > len(parsed):
            return json.loads(top), None
        return None, "no majority: " + " / ".join(canon(p) if p else "-" for p in parsed)
    if any(p is None for p in parsed):
        return None, "a sample gave no readable answer"
    if all(canon(p) == canon(parsed[0]) for p in parsed):
        return parsed[0], None
    return None, "samples disagree: " + " / ".join(canon(p) for p in parsed)


sys.stdin.readline()  # handshake
impl = a.impl or (f"{meta['label']}@{meta['model']}/{meta['effort']}/k{meta['samples']}/{a.policy}" if meta else "llm")
print(json.dumps({"ok": True, "impl": impl}, ensure_ascii=False), flush=True)

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    rec = table.get(canon(req["in"]))
    if rec is None:
        print(json.dumps({"id": req["id"], "err": "no answer recorded for this input"}, ensure_ascii=False), flush=True)
        continue
    out, err = decide(rec)
    if out is None:
        print(json.dumps({"id": req["id"], "err": err}, ensure_ascii=False), flush=True)
    else:
        print(json.dumps({"id": req["id"], "out": out}, ensure_ascii=False), flush=True)
