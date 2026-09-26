# Runs a graph that dandori generated for pydantic-graph against scripted answers, and prints
# what it did in the shape the reference interpreter prints for pydantic-graph: every call with
# its arguments and the answer it got, and how the run ended.
#
#   .venv/bin/python run.py <generated package> <runs.json> <results.json>
#
# runs.json: { "own": [ { "name", "method", "callback" } ], "rules": [ { "fn", "name" } ],
#              "http": [...], "aws": [...],
#              "runs": [ { "input": {...}, "answers": [ {"ok": value} | {"error": kind} ] } ] }
#
# The tasks that say `lambda`, `http` or `aws` run the generated code, with a Transport that
# writes down what it would send (../transport.py). The tasks the user writes and the rules
# are stand-ins that answer from the scenario. A callback's answer goes to Deps.callbacks; a
# callback that the scenario times out gets none. The clock skips ahead instead of sleeping. A
# task's own timeout cannot be scripted this way, so runs.json holds none of those.
#
# The rounds of `for … in parallel` run one at a time here, so that the calls come in the
# order the reference interpreter makes them.

from __future__ import annotations

import asyncio
import importlib
import json
import os
import re
import shutil
import sys
import tempfile
from datetime import datetime, timedelta, timezone
from typing import Any

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from transport import make_transport  # noqa: E402


class SkippingClock:
    """A clock whose sleeps pass at once, moving its time ahead."""

    def __init__(self) -> None:
        self.t = datetime.now(timezone.utc)

    def now(self) -> datetime:
        return self.t

    async def sleep(self, seconds: float) -> None:
        self.t += timedelta(seconds=seconds)
        await asyncio.sleep(0)


async def run_all(package: str, spec: dict[str, Any]) -> list[dict[str, Any]]:
    g = importlib.import_module(f"{package}.graph")
    tasks = importlib.import_module(f"{package}.tasks")
    rt = importlib.import_module(f"{package}.runtime")
    current: dict[str, Any] = {}

    def take(label: str) -> dict[str, Any]:
        if current["next"] >= len(current["answers"]):
            raise rt.TaskFailure("Dandori.Test.NoAnswer", f"no answer for call {current['next'] + 1} ({label})")
        ans = current["answers"][current["next"]]
        current["next"] += 1
        return ans

    def recorded(ans: dict[str, Any]) -> dict[str, Any]:
        return {"ok": ans["ok"]} if "ok" in ans else {"error": ans["error"], "as": ans["error"]}

    def scripted(kind: str) -> Exception:
        return rt.TaskFailure("Dandori.Test.Failure" if kind == "failure" else kind, "scripted")

    def answer_later(callback_id: str, ans: dict[str, Any]) -> None:
        """Answer a callback through Deps.callbacks; a timeout gets no answer."""
        if "ok" not in ans and ans["error"] == "timeout":
            return
        if "ok" in ans:
            answer = {"ok": ans["ok"]}
        else:
            answer = {"error": "Dandori.Test.Failure" if ans["error"] == "failure" else ans["error"], "message": "scripted"}
        current["callbacks"].answer(callback_id, answer)

    class Run:
        """The stand-in transport writes every call down with the answer it takes."""

        def take(self, call: dict[str, Any], callback_id: str | None = None) -> dict[str, Any]:
            ans = take(json.dumps(call, ensure_ascii=False)[:80])
            current["steps"].append({"call": call, "answer": recorded(ans)})
            return ans

        def answer_later(self, callback_id: str, ans: dict[str, Any]) -> None:
            answer_later(callback_id, ans)

    class Own:
        pass

    own = Own()
    for t in spec.get("own", []):

        def own_task(t: dict[str, Any]) -> Any:
            async def call(args: dict[str, Any]) -> Any:
                ans = take(t["method"])
                if t["callback"]:
                    rest = {k: v for k, v in args.items() if k != "callback_id"}
                    current["steps"].append({"call": {"callback": t["name"], "args": rest}, "answer": recorded(ans)})
                    answer_later(args["callback_id"], ans)
                    return None
                current["steps"].append({"call": {"task": t["name"], "args": args}, "answer": recorded(ans)})
                if "ok" in ans:
                    return ans["ok"]
                raise scripted(ans["error"])

            return call

        setattr(own, t["method"], own_task(t))

    class Rules:
        pass

    rules = Rules()
    for r in spec.get("rules", []):

        def rule(r: dict[str, Any]) -> Any:
            async def call(args: dict[str, Any]) -> Any:
                ans = take(r["name"])
                current["steps"].append({"call": {"rule": r["name"], "args": args}, "answer": recorded(ans)})
                if "ok" in ans:
                    return ans["ok"]
                raise scripted(ans["error"])

            return call

        setattr(rules, r["fn"], rule(r))

    results = []
    for r in spec["runs"]:
        callbacks = rt.Callbacks()
        current.clear()
        current.update({"answers": r["answers"], "next": 0, "steps": [], "callbacks": callbacks})
        deps: dict[str, Any] = {"tasks": tasks.make_tasks(own, make_transport(spec, Run())), "clock": SkippingClock(), "callbacks": callbacks, "run_id": "test"}
        if spec.get("rules"):
            deps["rules"] = rules
        try:
            out = await g.graph.run(inputs=r["input"], state=g.State(), deps=g.Deps(**deps))
            end: dict[str, Any] = {"succeed": out}
        except rt.Failure as e:
            end = {"fail": {"error": e.error, "cause": e.cause or None}}
        results.append({"steps": current["steps"], "end": end})
    return results


def main() -> None:
    package_dir, runs_file, out_file = sys.argv[1:4]
    with open(runs_file, encoding="utf-8") as f:
        spec = json.load(f)
    # a copy of the package, whose rounds of a parallel loop run one at a time
    work = tempfile.mkdtemp(prefix="dandori-pydantic-graph-")
    package = os.path.basename(os.path.normpath(package_dir))
    copy = os.path.join(work, package)
    shutil.copytree(package_dir, copy)
    path = os.path.join(copy, "graph.py")
    with open(path, encoding="utf-8") as f:
        text = re.sub(r"dd\.at_a_time\(\d+\)", "dd.at_a_time(1)", f.read())
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    sys.path.insert(0, work)
    try:
        results = asyncio.run(run_all(package, spec))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")


if __name__ == "__main__":
    main()
