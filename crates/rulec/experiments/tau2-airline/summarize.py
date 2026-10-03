#!/usr/bin/env python3
"""Compare τ²-bench runs: the average reward and pass^k of each, the tasks on which they
differ, and how often the rulec tools were called in the run that has them.

  python3 summarize.py without/results.json with/results.json
"""
import collections
import json
import sys

RULEC_TOOLS = ("baggage_fee", "cancellation_verdict", "compensation_amount")


def load(path):
    d = json.load(open(path, encoding="utf-8"))
    by_task = collections.defaultdict(list)
    calls = collections.Counter()
    for s in d.get("simulations", []):
        r = s.get("reward_info") or {}
        reward = r.get("reward", s.get("reward"))
        by_task[str(s.get("task_id"))].append(float(reward) if reward is not None else 0.0)
        for m in s.get("messages", []):
            for tc in m.get("tool_calls") or []:
                n = tc.get("name") or (tc.get("function") or {}).get("name")
                if n in RULEC_TOOLS:
                    calls[n] += 1
    return d, by_task, calls


def pass_hat_k(trials, k):
    # the probability that k trials drawn without replacement all pass (τ-bench's pass^k)
    n, c = len(trials), sum(1 for t in trials if t >= 1.0)
    if n < k:
        return None
    from math import comb
    return comb(c, k) / comb(n, k)


def summary(by_task):
    tasks = sorted(by_task, key=lambda t: int(t) if t.isdigit() else t)
    rewards = [r for t in tasks for r in by_task[t]]
    avg = sum(rewards) / len(rewards) if rewards else 0.0
    ks = {}
    for k in (1, 2, 3, 4):
        vals = [pass_hat_k(by_task[t], k) for t in tasks]
        vals = [v for v in vals if v is not None]
        if vals:
            ks[k] = sum(vals) / len(vals)
    return tasks, avg, ks


runs = [load(p) for p in sys.argv[1:]]
names = ["without tools", "with tools"][: len(runs)] + [f"run {i}" for i in range(2, len(runs))]
print("| run | tasks | trials | avg reward | " + " | ".join(f"pass^{k}" for k in (1, 2, 3, 4)) + " |")
print("|---|---|---|---|---|---|---|---|")
sums = []
for name, (_, by_task, calls) in zip(names, runs):
    tasks, avg, ks = summary(by_task)
    trials = max((len(v) for v in by_task.values()), default=0)
    sums.append((tasks, by_task))
    print(f"| {name} | {len(tasks)} | {trials} | {avg:.3f} | " + " | ".join(f"{ks[k]:.3f}" if k in ks else "" for k in (1, 2, 3, 4)) + " |")
for name, (_, _, calls) in zip(names, runs):
    if calls:
        print(f"\n{name}: rulec tools called " + ", ".join(f"{n} ×{c}" for n, c in calls.most_common()))
if len(runs) >= 2:
    a, b = sums[0][1], sums[1][1]
    diff = [(t, sum(a[t]) / len(a[t]), sum(b[t]) / len(b[t])) for t in a if t in b and sum(a[t]) / len(a[t]) != sum(b[t]) / len(b[t])]
    if diff:
        print("\n| task | without | with |\n|---|---|---|")
        for t, x, y in sorted(diff, key=lambda d: int(d[0]) if d[0].isdigit() else d[0]):
            print(f"| {t} | {x:.2f} | {y:.2f} |")
    else:
        print("\nno task differs between the two runs")
