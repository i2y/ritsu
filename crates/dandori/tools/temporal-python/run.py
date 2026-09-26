# Runs a workflow that dandori generated for Temporal's Python SDK on a real Temporal server — the
# dev server of the Temporal CLI, which the SDK's testing package starts — against scripted
# answers, and prints what it did in the shape the reference interpreter prints for Temporal:
# every call with its arguments and the answer it got, and how the workflow ended. The Python twin
# of ../temporal/run.mjs.
#
#   .venv/bin/python run.py <generated package> <runs.json> <results.json>
#
# runs.json: { "workflow": type, "own": [ { "name", "method", "callback" } ], "rules": [activity],
#              "children": [ { "type", "queue" } ], "queues": [queue], "http": [...], "aws": [...],
#              "runs": [ { "id": workflow id, "input": {...}, "answers": [ {"ok": value} | {"error": kind} ] } ] }
#
# Every run goes at once, each as the workflow with its own id; a stand-in finds its run by the id
# of the workflow that called it (a child workflow's id starts with its parent's). The tasks that
# say `lambda`, `http` or `aws` run the generated code, with a Transport that writes down what it
# would send (../transport.py). The tasks the user writes, the rules and the child workflows are
# stand-ins that answer from the scenario. A callback's answer comes as the signal the workflow
# waits for, sent before the task that hands the id on returns; a callback that the scenario
# times out gets none. A call that the scenario times out gets no answer either: its stand-in
# keeps the activity busy until the server times it out.
#
# The server keeps real time, so the copy of the generated code that runs here waits far less:
# every duration of the workflow's timers (dd.seconds) is at most 10 ms, and an activity or a
# child workflow gets 2 seconds before it times out. The rounds of `for … in parallel` run one at
# a time here, so that the calls come in the order the reference interpreter makes them.

from __future__ import annotations

import asyncio
import contextlib
import importlib
import json
import logging
import os
import re
import shutil
import sys
import tempfile
import time
from typing import Any

from temporalio import activity
from temporalio.client import WorkflowFailureError
from temporalio.exceptions import ApplicationError, CancelledError
from temporalio.runtime import LoggingConfig, Runtime, TelemetryConfig, TelemetryFilter
from temporalio.testing import WorkflowEnvironment
from temporalio.worker import Worker

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from transport import make_transport  # noqa: E402

# the workers' log goes quiet but for errors; the results go to a file
logging.basicConfig(level=logging.ERROR)
Runtime.set_default(Runtime(telemetry=TelemetryConfig(logging=LoggingConfig(filter=TelemetryFilter(core_level="ERROR", other_level="ERROR")))), error_if_already_set=True)

# How long an activity or a child workflow gets here before it times out, and how long a stand-in
# that the scenario times out keeps its activity busy.
TIMEOUT = 2
LATE = 5.0


def child_stand_ins(children: list[dict[str, Any]]) -> str:
    """The child workflows: stand-ins that ask an activity for the scenario's answer."""
    out = [
        "from __future__ import annotations",
        "",
        "from datetime import timedelta",
        "from typing import Any",
        "",
        "from temporalio import workflow",
        "from temporalio.common import RetryPolicy",
        "from temporalio.exceptions import ActivityError, ApplicationError",
        "",
    ]
    for i, c in enumerate(children):
        out += [
            "",
            f"@workflow.defn(name={json.dumps(c['type'], ensure_ascii=False)})",
            f"class Child{i}:",
            "    @workflow.run",
            "    async def run(self, input: Any) -> Any:",
            "        try:",
            f"            return await workflow.execute_activity(\"dd_test_child\", {{\"type\": {json.dumps(c['type'], ensure_ascii=False)}, \"args\": input}}, start_to_close_timeout=timedelta(seconds=60), retry_policy=RetryPolicy(maximum_attempts=1))",
            "        except ActivityError as e:",
            "            if isinstance(e.cause, ApplicationError):",
            "                raise ApplicationError(e.cause.message, type=e.cause.type, non_retryable=True) from None",
            "            raise",
            "",
        ]
    out.append(f"children = [{', '.join(f'Child{i}' for i in range(len(children)))}]")
    return "\n".join(out) + "\n"


async def run_all(package: str, spec: dict[str, Any]) -> list[dict[str, Any]]:
    wf = importlib.import_module(f"{package}.workflow")
    acts = importlib.import_module(f"{package}.activities")
    children = importlib.import_module(f"{package}.dd_test_children").children
    runs: dict[str, dict[str, Any]] = {}
    env: WorkflowEnvironment | None = None

    def current() -> dict[str, Any]:
        """The run of the activity being served: its workflow's id, or its parent's."""
        wid = activity.info().workflow_id
        return runs[wid.split("/")[0]]

    def take(run: dict[str, Any], label: str) -> dict[str, Any]:
        if run["next"] >= len(run["answers"]):
            raise ApplicationError(f"no answer for call {run['next'] + 1} ({label})", type="Dandori.Test.NoAnswer", non_retryable=True)
        ans = run["answers"][run["next"]]
        run["next"] += 1
        return ans

    def recorded(ans: dict[str, Any]) -> dict[str, Any]:
        return {"ok": ans["ok"]} if "ok" in ans else {"error": ans["error"], "as": ans["error"]}

    def scripted(kind: str) -> ApplicationError:
        return ApplicationError("scripted", type="Dandori.Test.Failure" if kind == "failure" else kind, non_retryable=True)

    async def late() -> Any:
        """A call the scenario times out: keep the activity busy past its timeout, heartbeating, so that the server times it out."""
        until = time.monotonic() + LATE
        while time.monotonic() < until:
            activity.heartbeat()
            await asyncio.sleep(0.1)
        raise ApplicationError("the server should have timed this out", type="Dandori.Test.Late", non_retryable=True)

    async def answer(ans: dict[str, Any]) -> Any:
        """The answer of a stand-in: its value, the scripted error, or no answer in time."""
        if "ok" in ans:
            return ans["ok"]
        if ans["error"] == "timeout":
            return await late()
        raise scripted(ans["error"])

    async def answer_later(callback_id: str, ans: dict[str, Any]) -> None:
        """Answer a callback with a signal to the workflow the id names, before the task that
        hands the id on returns: then the answer is in the history before the workflow waits for
        it, and its short wait cannot run out first. A timeout gets no answer."""
        if "ok" not in ans and ans["error"] == "timeout":
            return
        workflow_id = json.loads(callback_id)[0]
        if "ok" in ans:
            signal = {"callback_id": callback_id, "ok": ans["ok"]}
        else:
            signal = {"callback_id": callback_id, "error": "Dandori.Test.Failure" if ans["error"] == "failure" else ans["error"], "message": "scripted"}
        assert env is not None
        await env.client.get_workflow_handle(workflow_id).signal("dandori.callback", signal)

    class Run:
        """The stand-in transport writes every call down with the answer it takes; a call it times out gets none."""

        def take(self, call: dict[str, Any], callback_id: str | None = None) -> dict[str, Any]:
            run = current()
            ans = take(run, json.dumps(call, ensure_ascii=False)[:80])
            run["steps"].append({"call": call, "answer": recorded(ans)})
            return ans

        def answer_later(self, callback_id: str, ans: dict[str, Any]) -> Any:
            return answer_later(callback_id, ans)

        async def late(self) -> Any:
            return await late()

    class Own:
        pass

    own = Own()
    for t in spec.get("own", []):

        def own_task(t: dict[str, Any]) -> Any:
            async def call(args: dict[str, Any]) -> Any:
                run = current()
                ans = take(run, t["method"])
                if t["callback"]:
                    rest = {k: v for k, v in args.items() if k != "callback_id"}
                    run["steps"].append({"call": {"activity": t["name"], "args": rest}, "answer": recorded(ans)})
                    await answer_later(args["callback_id"], ans)
                    return None
                run["steps"].append({"call": {"activity": t["name"], "args": args}, "answer": recorded(ans)})
                return await answer(ans)

            return call

        setattr(own, t["method"], own_task(t))

    activities = list(acts.make_activities(own, make_transport(spec, Run())))
    for name in spec.get("rules", []):

        def rule(name: str) -> Any:
            @activity.defn(name=name)
            async def call(args: dict[str, Any]) -> Any:
                run = current()
                ans = take(run, name)
                run["steps"].append({"call": {"activity": name, "args": args}, "answer": recorded(ans)})
                return await answer(ans)

            return call

        activities.append(rule(name))

    @activity.defn(name="dd_test_child")
    async def test_child(a: dict[str, Any]) -> Any:
        run = current()
        ans = take(run, a["type"])
        run["steps"].append({"call": {"child_workflow": a["type"], "args": a["args"]}, "answer": recorded(ans)})
        return await answer(ans)

    activities.append(test_child)

    # the server fires a timer up to a second late unless told to shift its timers less
    env = await WorkflowEnvironment.start_local(ui=False, dev_server_log_level="error", dev_server_extra_args=["--dynamic-config-value", 'history.timerProcessorMaxTimeShift="10ms"'])
    results = []
    try:
        queues = ["dandori", *sorted({q for q in spec.get("queues", []) if q != "dandori"})]
        async with contextlib.AsyncExitStack() as stack:
            # every worker runs until the runs are done
            for q in queues:
                await stack.enter_async_context(Worker(env.client, task_queue=q, workflows=[*wf.workflows, *children], activities=activities, max_concurrent_activities=200))

            async def one(r: dict[str, Any]) -> dict[str, Any]:
                runs[r["id"]] = {"answers": r["answers"], "next": 0, "steps": []}
                try:
                    out = await env.client.execute_workflow(spec["workflow"], r["input"], id=r["id"], task_queue="dandori")
                    return {"succeed": out}
                except WorkflowFailureError as e:
                    c = e.cause
                    if isinstance(c, CancelledError):
                        return {"cancel": None}
                    # the message as the workflow sent it: the SDK reads an empty one as "Application error"
                    raw = getattr(c, "failure", None)
                    message = raw.message if raw is not None else getattr(c, "message", None)
                    return {"fail": {"error": getattr(c, "type", None) or str(c), "cause": message or None}}

            ends = await asyncio.gather(*(one(r) for r in spec["runs"]))
            for r, end in zip(spec["runs"], ends):
                results.append({"steps": runs[r["id"]]["steps"], "end": end})
    finally:
        await env.shutdown()
    return results


def main() -> None:
    package_dir, runs_file, out_file = sys.argv[1:4]
    with open(runs_file, encoding="utf-8") as f:
        spec = json.load(f)
    # a copy of the package: the rounds of a parallel loop run one at a time, the timers are
    # short, so are the activities' and the child workflows' timeouts, and the child workflows'
    # stand-ins sit beside the workflow
    work = tempfile.mkdtemp(prefix="dandori-temporal-python-")
    package = os.path.basename(os.path.normpath(package_dir))
    copy = os.path.join(work, package)
    shutil.copytree(package_dir, copy)
    path = os.path.join(copy, "workflow.py")
    with open(path, encoding="utf-8") as f:
        text = re.sub(r"dd\.at_a_time\(\d+\)", "dd.at_a_time(1)", f.read())
    text = re.sub(r"start_to_close_timeout=timedelta\(seconds=\d+\)", f"start_to_close_timeout=timedelta(seconds={TIMEOUT})", text)
    text = re.sub(r"execution_timeout=timedelta\(seconds=\d+\)", f"execution_timeout=timedelta(seconds={TIMEOUT})", text)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    path = os.path.join(copy, "runtime.py")
    with open(path, encoding="utf-8") as f:
        text = f.read()
    marker = '"""A duration of the workflow\'s timers, in seconds: every wait of the workflow goes through here."""\n    return n\n'
    assert marker in text, "runtime.py has no seconds() to shorten"
    text = text.replace(marker, marker.replace("return n", "return min(n, 0.01)"))
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    with open(os.path.join(copy, "dd_test_children.py"), "w", encoding="utf-8") as f:
        f.write(child_stand_ins(spec.get("children", [])))
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
