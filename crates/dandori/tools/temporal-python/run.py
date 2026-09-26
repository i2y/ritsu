# Runs a workflow that dandori generated for Temporal's Python SDK on a real Temporal server — the
# dev server of the Temporal CLI, which the SDK's testing package starts — against scripted
# answers, and prints what it did in the shape the reference interpreter prints for Temporal:
# every call with its arguments and the answer it got, and how the workflow ended. With each run,
# it also prints what the query dandori.status says of the cases at the end (`cases`), and the
# search attribute DandoriCases (`shown`). The workers, the starts, the callbacks' answers and the
# query go through the generated worker.py and client.py. The Python twin of ../temporal/run.mjs.
#
#   .venv/bin/python run.py <generated package> <runs.json> <results.json> [<histories dir>]
#   .venv/bin/python run.py --replay <generated package> <histories dir> <results.json>
#
# Once the runs are over, the history of each is replayed with the same code, which must not find
# it nondeterministic; with a histories directory, the histories are also written there as JSON,
# one file a run: <workflow id>.json, and for each run that went on from it after a
# Continue-As-New, <workflow id>.<n>.json (the second is 2). With --replay, it only replays the
# histories of a directory (written by this runner, or by the TypeScript one) with the code, and
# writes for each file the error, or None.
#
# runs.json: { "workflow": type, "own": [ { "name", "method", "callback" } ], "rules": [activity],
#              "children": [ { "type", "queue" } ], "queues": [queue], "http": [...], "aws": [...],
#              "runs": [ { "id": workflow id, "input": {...},
#                          "answers": [ {"ok": value} | {"error": kind} | {"cancel": true} ] } ] }
#
# Every run goes at once, each as the workflow with its own id; a stand-in finds its run by the id
# of the workflow that called it (a child workflow's id starts with its parent's). The tasks that
# say `lambda`, `http` or `aws` run the generated code, with a Transport that writes down what it
# would send (../transport.py). The tasks the user writes, the rules and the child workflows are
# stand-ins that answer from the scenario. A callback's answer comes as the signal the workflow
# waits for, sent before the task that hands the id on returns; a callback that the scenario
# times out gets none. A call that the scenario times out gets no answer either: its stand-in
# keeps the activity busy until the server times it out. A call during which the scenario
# cancels the workflow asks the server to cancel it, and keeps the activity busy until the server
# tells it that it is cancelled; a callback's answer that is a cancellation is the request to
# cancel.
#
# The server keeps real time, so the copy of the generated code that runs here waits far less:
# every duration of the workflow's timers (dd.seconds) is at most 10 ms, and an activity or a
# child workflow gets 2 seconds before it times out. The rounds of `for … in parallel` run one at
# a time here, so that the calls come in the order the reference interpreter makes them. A loop at
# the top of the flow goes on in a new run (Continue-As-New) at every round but the first of a
# run, since the history counts as long from one event on here (dd.CONTINUE_AT).

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
from datetime import timedelta
from typing import Any

from temporalio import activity
from temporalio.client import WorkflowFailureError, WorkflowUpdateFailedError
from temporalio.common import SearchAttributeKey
from temporalio.exceptions import ApplicationError, CancelledError
from temporalio.runtime import LoggingConfig, Runtime, TelemetryConfig, TelemetryFilter
from temporalio.client import WorkflowHistory
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


def workflow_id_of(file: str) -> str:
    """The workflow id of a history's file: <workflow id>.json, or <workflow id>.<n>.json for a run that went on from it."""
    return re.sub(r"(\.\d+)?\.json$", "", file)


async def runs_of(client: Any, workflow_id: str, first_run_id: str | None) -> list[WorkflowHistory]:
    """The histories of a workflow's runs, from the first: a run that went on in a new one (Continue-As-New) is followed by that one."""
    out = []
    run_id = first_run_id
    while run_id:
        history = await client.get_workflow_handle(workflow_id, run_id=run_id).fetch_history()
        out.append(history)
        last = history.events[-1]
        run_id = last.workflow_execution_continued_as_new_event_attributes.new_execution_run_id if last.HasField("workflow_execution_continued_as_new_event_attributes") else None
    return out


async def replay(package: str, histories: list[Any]) -> dict[str, str | None]:
    """Replay histories with the generated worker's replay: for each workflow, the error of a run of it that it finds nondeterministic, or None."""
    worker = importlib.import_module(f"{package}.worker")
    why = dict(await worker.replay(histories))
    return {h.workflow_id: why.get(h.workflow_id) for h in histories}


async def run_all(package: str, spec: dict[str, Any], histories_dir: str | None) -> list[dict[str, Any]]:
    children = importlib.import_module(f"{package}.dd_test_children").children
    worker = importlib.import_module(f"{package}.worker")
    client = importlib.import_module(f"{package}.client")
    cases_key = SearchAttributeKey.for_keyword_list("DandoriCases")
    refusals_checked = False
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
        if ans.get("cancel") is True:
            return {"cancel": True}
        return {"ok": ans["ok"]} if "ok" in ans else {"error": ans["error"], "as": ans["error"]}

    def scripted(kind: str) -> ApplicationError:
        return ApplicationError("scripted", type="Dandori.Test.Failure" if kind == "failure" else kind, non_retryable=True)

    async def hold_on() -> None:
        """Heartbeat until the server says the activity is over (cancelled, or timed out), at most LATE seconds."""
        until = time.monotonic() + LATE
        while time.monotonic() < until:
            activity.heartbeat()
            try:
                await asyncio.sleep(0.1)
            except asyncio.CancelledError:
                return

    async def late() -> Any:
        """A call the scenario times out: keep the activity busy past its timeout, so that the server times it out."""
        await hold_on()
        raise ApplicationError("the server should have timed this out", type="Dandori.Test.Late", non_retryable=True)

    async def cancelling(run: dict[str, Any]) -> Any:
        """A call during which the scenario cancels the workflow: ask the server, and keep the activity until it is cancelled."""
        assert env is not None
        await env.client.get_workflow_handle(run["id"]).cancel()
        await hold_on()
        raise ApplicationError("the workflow was cancelled", type="Dandori.Test.Cancelled", non_retryable=True)

    async def answer(run: dict[str, Any], ans: dict[str, Any]) -> Any:
        """The answer of a stand-in: its value, the scripted error, no answer in time, or none before the workflow is cancelled."""
        if ans.get("cancel") is True:
            return await cancelling(run)
        if "ok" in ans:
            return ans["ok"]
        if ans["error"] == "timeout":
            return await late()
        raise scripted(ans["error"])

    async def must_refuse(callback_id: str, a: dict[str, Any], why: str) -> None:
        """A second answer to a callback, and an answer to one the workflow never waits for, must be refused."""
        assert env is not None
        try:
            await client.answer(env.client, callback_id, a)
        except WorkflowUpdateFailedError:
            return
        raise RuntimeError(f"the workflow took {why}")

    async def answer_later(callback_id: str, ans: dict[str, Any]) -> None:
        """Answer a callback with the generated client's update, before the task that hands the
        id on returns: then the answer is in the history before the workflow waits for it, and its
        short wait cannot run out first. A timeout gets no answer."""
        nonlocal refusals_checked
        if "ok" not in ans and ans.get("error") == "timeout":
            return
        assert env is not None
        if ans.get("cancel") is True:
            await env.client.get_workflow_handle(current()["id"]).cancel()
            return
        if "ok" in ans:
            a = {"ok": ans["ok"]}
        else:
            a = {"error": "Dandori.Test.Failure" if ans["error"] == "failure" else ans["error"], "message": "scripted"}
        await client.answer(env.client, callback_id, a)
        await must_refuse(callback_id, a, "a second answer to a callback")
        if not refusals_checked:
            refusals_checked = True
            workflow_id = json.loads(callback_id)[0]
            await must_refuse(json.dumps([workflow_id, "0"]), a, "an answer to a callback it does not wait for")

    class Run:
        """The stand-in transport writes every call down with the answer it takes; a call it times out gets none."""

        def take(self, call: dict[str, Any], callback_id: str | None = None) -> dict[str, Any]:
            run = current()
            ans = take(run, json.dumps(call, ensure_ascii=False)[:80])
            run["steps"].append({"call": call, "answer": recorded(ans)})
            return ans

        def answer_later(self, callback_id: str, ans: dict[str, Any]) -> Any:
            return answer_later(callback_id, ans)

        async def hold(self, ans: dict[str, Any]) -> Any:
            """A call the runner keeps from answering: one that times out, or one during which the workflow is cancelled."""
            if ans.get("cancel") is True:
                return await cancelling(current())
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
                return await answer(run, ans)

            return call

        setattr(own, t["method"], own_task(t))

    activities: list[Any] = []
    for name in spec.get("rules", []):

        def rule(name: str) -> Any:
            @activity.defn(name=name)
            async def call(args: dict[str, Any]) -> Any:
                run = current()
                ans = take(run, name)
                run["steps"].append({"call": {"activity": name, "args": args}, "answer": recorded(ans)})
                return await answer(run, ans)

            return call

        activities.append(rule(name))

    @activity.defn(name="dd_test_child")
    async def test_child(a: dict[str, Any]) -> Any:
        run = current()
        ans = take(run, a["type"])
        run["steps"].append({"call": {"child_workflow": a["type"], "args": a["args"]}, "answer": recorded(ans)})
        return await answer(run, ans)

    activities.append(test_child)

    # the server fires a timer up to a second late unless told to shift its timers less
    env = await WorkflowEnvironment.start_local(
        ui=False,
        dev_server_log_level="error",
        dev_server_extra_args=["--dynamic-config-value", 'history.timerProcessorMaxTimeShift="10ms"'],
        search_attributes=[cases_key],
    )
    results = []
    try:
        base = worker.worker_options(own, transport=make_transport(spec, Run()))
        base["workflows"] = [*base["workflows"], *children]
        base["activities"] = [*base["activities"], *activities]
        queues = [client.TASK_QUEUE, *sorted({q for q in spec.get("queues", []) if q != client.TASK_QUEUE})]
        async with contextlib.AsyncExitStack() as stack:
            # every worker runs until the runs are done
            for q in queues:
                # heartbeats go out at once, so that a held activity hears soon that it is over
                await stack.enter_async_context(
                    Worker(
                        env.client,
                        **{
                            **base,
                            "task_queue": q,
                            "max_concurrent_activities": 200,
                            "default_heartbeat_throttle_interval": timedelta(milliseconds=100),
                            "max_heartbeat_throttle_interval": timedelta(milliseconds=100),
                        },
                    )
                )

            async def one(r: dict[str, Any]) -> dict[str, Any]:
                runs[r["id"]] = {"id": r["id"], "answers": r["answers"], "next": 0, "steps": []}
                handle = await client.start(env.client, r["id"], r["input"], search_attributes=True)
                end: dict[str, Any]
                try:
                    end = {"succeed": await handle.result()}
                except WorkflowFailureError as e:
                    c = e.cause
                    if isinstance(c, CancelledError):
                        end = {"cancel": None}
                    else:
                        # the message as the workflow sent it: the SDK reads an empty one as "Application error"
                        raw = getattr(c, "failure", None)
                        message = raw.message if raw is not None else getattr(c, "message", None)
                        end = {"fail": {"error": getattr(c, "type", None) or str(c), "cause": message or None}}
                # what the query and the search attribute say of the cases once the run is over
                cases = (await client.status(env.client, r["id"]))["cases"]
                shown = (await handle.describe()).typed_search_attributes.get(cases_key)
                return {"end": end, "cases": cases, "shown": list(shown) if shown is not None else None, "histories": await runs_of(env.client, r["id"], handle.first_execution_run_id)}

            ends = await asyncio.gather(*(one(r) for r in spec["runs"]))
            histories = []
            for got in ends:
                for n, h in enumerate(got.pop("histories")):
                    histories.append((h.workflow_id + (".json" if n == 0 else f".{n + 1}.json"), h))
            for r, got in zip(spec["runs"], ends):
                results.append({"steps": runs[r["id"]]["steps"], **got})
            # the generated client finds every run by the workflow's type, the ones that went on in a new run too (the server lists them a moment late)
            for tries in range(51):
                found = [h.workflow_id async for h in client.histories(env.client, f"WorkflowType = '{client.WORKFLOW_TYPE}'")]
                if len(found) == len(histories):
                    break
                if tries == 50:
                    raise RuntimeError(f"the client's histories found {len(found)} run(s) of {len(histories)}")
                await asyncio.sleep(0.1)
        # the same code, replaying what it did, must find it deterministic
        for wid, error in (await replay(package, [h for _, h in histories])).items():
            if error is not None:
                raise RuntimeError(f"replaying the history of {wid} with the same code: {error}")
        if histories_dir is not None:
            os.makedirs(histories_dir, exist_ok=True)
            for name, h in histories:
                with open(os.path.join(histories_dir, name), "w", encoding="utf-8") as f:
                    f.write(h.to_json() + "\n")
            with open(os.path.join(histories_dir, "spec.json"), "w", encoding="utf-8") as f:
                json.dump({"children": spec.get("children", [])}, f, ensure_ascii=False)
    finally:
        await env.shutdown()
    return results


def main() -> None:
    replaying = sys.argv[1] == "--replay"
    if replaying:
        package_dir, histories_dir, out_file = sys.argv[2:5]
        with open(os.path.join(histories_dir, "spec.json"), encoding="utf-8") as f:
            spec = json.load(f)
    else:
        package_dir, runs_file, out_file = sys.argv[1:4]
        histories_dir = sys.argv[4] if len(sys.argv) > 4 else None
        with open(runs_file, encoding="utf-8") as f:
            spec = json.load(f)
    # a copy of the package: the rounds of a parallel loop run one at a time, the timers are
    # short, so are the activities' and the child workflows' timeouts, every history is long
    # enough to go on in a new run, and the child workflows' stand-ins sit beside the workflow
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
    assert "\nCONTINUE_AT = 10000\n" in text, "runtime.py has no CONTINUE_AT to lower"
    text = text.replace("\nCONTINUE_AT = 10000\n", "\nCONTINUE_AT = 1\n")
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    with open(os.path.join(copy, "dd_test_children.py"), "w", encoding="utf-8") as f:
        f.write(child_stand_ins(spec.get("children", [])))
    # the rules are stand-ins here, so the worker needs none of rulec's modules
    if os.path.exists(os.path.join(copy, "rules.py")):
        with open(os.path.join(copy, "rules.py"), "w", encoding="utf-8") as f:
            f.write("rules: list = []\n")
    sys.path.insert(0, work)
    try:
        if replaying:
            files = sorted(f for f in os.listdir(histories_dir) if f.endswith(".json") and f != "spec.json")
            histories = []
            for name in files:
                with open(os.path.join(histories_dir, name), encoding="utf-8") as f:
                    histories.append(WorkflowHistory.from_json(workflow_id_of(name), f.read()))
            got = asyncio.run(replay(package, histories))
            results: Any = [{"file": name, "error": got.get(workflow_id_of(name), "not replayed")} for name in files]
        else:
            results = asyncio.run(run_all(package, spec, histories_dir))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")


if __name__ == "__main__":
    main()
