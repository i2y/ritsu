# Worker Deployment Versioning on a real Temporal server (the dev server): two builds of one
# workflow, A and B, whose workers are versions of one deployment through the generated
# worker.py. A run that starts on A and waits while B becomes the current version must end on A's
# code; a run that starts after must run on B's. The Python twin of ../temporal/versions.mjs.
#
#   .venv/bin/python versions.py <generated package A> <generated package B> <results.json>
#
# results.json: { "buildIds": [A's, B's], "notified": { "<run>": [the texts it notified] } }

from __future__ import annotations

import asyncio
import importlib
import json
import logging
import os
import shutil
import sys
import tempfile
from typing import Any, Awaitable, Callable

from temporalio import activity
from temporalio.api.workflowservice.v1 import SetWorkerDeploymentCurrentVersionRequest
from temporalio.runtime import LoggingConfig, Runtime, TelemetryConfig, TelemetryFilter
from temporalio.testing import WorkflowEnvironment
from temporalio.worker import Worker

logging.basicConfig(level=logging.ERROR)
Runtime.set_default(Runtime(telemetry=TelemetryConfig(logging=LoggingConfig(filter=TelemetryFilter(core_level="ERROR", other_level="ERROR")))), error_if_already_set=True)

DEPLOYMENT = "dandori-versions"

# what each run did: the ids of its callbacks, and the texts it notified
callbacks: dict[str, str] = {}
notified: dict[str, list[str]] = {}


class Own:
    """The tasks of examples/review, as the scenario wants them: scoring holds each application."""

    async def 審査する(self, args: dict[str, Any]) -> Any:
        return {"点数": 50, "判断": "保留"}

    async def 承認を求める(self, args: dict[str, Any]) -> None:
        callbacks[activity.info().workflow_id] = args["callback_id"]

    async def 通知する(self, args: dict[str, Any]) -> None:
        notified.setdefault(activity.info().workflow_id, []).append(args["本文"])


async def until(what: str, ok: Callable[[], Awaitable[Any]]) -> Any:
    for _ in range(300):
        got = await ok()
        if got:
            return got
        await asyncio.sleep(0.1)
    raise RuntimeError(f"gave up waiting until {what}")


async def main(packages: list[str], out_file: str) -> None:
    env = await WorkflowEnvironment.start_local(ui=False, dev_server_log_level="error", dev_server_extra_args=["--dynamic-config-value", 'history.timerProcessorMaxTimeShift="10ms"'])
    try:
        builds = [(importlib.import_module(f"{p}.worker"), importlib.import_module(f"{p}.client")) for p in packages]
        (worker_a, client_a), (worker_b, client_b) = builds

        async def current(worker: Any) -> None:
            """Make the build the current version of the deployment, once the server has seen its worker."""

            async def set_it() -> bool:
                try:
                    await env.client.workflow_service.set_worker_deployment_current_version(
                        SetWorkerDeploymentCurrentVersionRequest(namespace="default", deployment_name=DEPLOYMENT, build_id=worker.BUILD_ID)
                    )
                    return True
                except Exception:  # noqa: BLE001 - the server has not seen the worker's pollers yet
                    return False

            await until(f"{worker.BUILD_ID} can be the current version", set_it)

        inputs = {"申込": {"id": "申込-1", "金額": 1000}}
        # no workflow kept in the workers' cache: a run goes where the server sends it, not where it was
        async with Worker(env.client, **worker_a.worker_options(Own(), deployment=DEPLOYMENT), max_cached_workflows=0):
            await current(worker_a)
            first = await client_a.start(env.client, "run-a", inputs)

            async def first_waits() -> Any:
                return callbacks.get("run-a")

            await until("the first run waits for its approval", first_waits)
            async with Worker(env.client, **worker_b.worker_options(Own(), deployment=DEPLOYMENT), max_cached_workflows=0):
                await current(worker_b)
                second = await client_b.start(env.client, "run-b", inputs)

                async def second_waits() -> Any:
                    return callbacks.get("run-b")

                await until("the second run waits for its approval", second_waits)
                # the first run, pinned to A, goes on with A's code; the second, started on B, with B's
                await client_a.answer(env.client, callbacks["run-a"], {"ok": {"承認者": "a"}})
                await client_b.answer(env.client, callbacks["run-b"], {"ok": {"承認者": "b"}})
                await asyncio.gather(first.result(), second.result())
        out = {"buildIds": [worker_a.BUILD_ID, worker_b.BUILD_ID], "notified": notified}
    finally:
        await env.shutdown()
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)
        f.write("\n")


if __name__ == "__main__":
    dir_a, dir_b, out_file = sys.argv[1:4]
    # the two builds are two packages of their own
    work = tempfile.mkdtemp(prefix="dandori-versions-")
    names = []
    for i, d in enumerate([dir_a, dir_b]):
        name = f"build_{'ab'[i]}"
        shutil.copytree(d, os.path.join(work, name))
        names.append(name)
    sys.path.insert(0, work)
    try:
        asyncio.run(main(names, out_file))
    finally:
        shutil.rmtree(work, ignore_errors=True)
