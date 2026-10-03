# A workflow and the .flow it runs as its child (`flow "<path>"`), both as dandori writes them, on
# one Temporal server (the dev server): the parent's worker and the child's, each on its own task
# queue, made by their generated worker.py. The Python twin of ../temporal/children.mjs: nothing
# stands in for the child, and with DANDORI_CHILD_BY (a JSON command, to which the server's address
# is added) the other language's runner serves the child until its input closes.
#
#   .venv/bin/python children.py <parent's generated package> <child's generated package> <runs.json> <results.json>
#   .venv/bin/python children.py --serve <child's generated package> <address>
#
# runs.json: [ { "id", "input" } ]
# results.json: [ { "end": { "succeed": the outputs } | { "fail": { "error", "cause" } } } ]

from __future__ import annotations

import asyncio
import importlib
import json
import logging
import os
import shutil
import sys
import tempfile
from typing import Any

from temporalio.client import Client, WorkflowFailureError
from temporalio.runtime import LoggingConfig, Runtime, TelemetryConfig, TelemetryFilter
from temporalio.testing import WorkflowEnvironment
from temporalio.worker import Worker

logging.basicConfig(level=logging.ERROR)
Runtime.set_default(Runtime(telemetry=TelemetryConfig(logging=LoggingConfig(filter=TelemetryFilter(core_level="ERROR", other_level="ERROR")))), error_if_already_set=True)


class NoTasks:
    """The flows here call nothing but each other, so no task is the user's."""


def worker(client: Client, package: str) -> Worker:
    """A build's worker, on its own task queue."""
    return Worker(client, **importlib.import_module(f"{package}.worker").worker_options(NoTasks()))


async def serve(package: str, address: str) -> None:
    """Serve the child on a server another runner started, until this process's input closes."""
    client = await Client.connect(address)
    async with worker(client, package):
        print("serving", flush=True)
        await asyncio.to_thread(sys.stdin.read)


async def main(parent: str, child: str | None, runs_file: str, out_file: str) -> None:
    with open(runs_file, encoding="utf-8") as f:
        runs = json.load(f)
    child_by = json.loads(os.environ["DANDORI_CHILD_BY"]) if "DANDORI_CHILD_BY" in os.environ else None
    env = await WorkflowEnvironment.start_local(ui=False, dev_server_log_level="error")
    results: list[dict[str, Any]] = []
    try:
        elsewhere = None
        if child_by is not None:
            # the other language's runner serves the child; its workers poll before the first run starts
            elsewhere = await asyncio.create_subprocess_exec(*child_by, env.client.service_client.config.target_host, stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE)
            assert elsewhere.stdout is not None
            while b"serving" not in await elsewhere.stdout.readline():
                if elsewhere.returncode is not None:
                    raise RuntimeError(f"the child's runner exited with {elsewhere.returncode}")
        start = importlib.import_module(f"{parent}.client").start
        workers = [worker(env.client, parent)] + ([worker(env.client, child)] if child is not None else [])
        for w in workers:
            await w.__aenter__()
        try:
            for r in runs:
                handle = await start(env.client, r["id"], r["input"])
                try:
                    end: dict[str, Any] = {"succeed": await handle.result()}
                except WorkflowFailureError as e:
                    c = e.cause
                    end = {"fail": {"error": getattr(c, "type", None) or str(c), "cause": getattr(c, "message", None) or None}}
                results.append({"end": end})
        finally:
            for w in reversed(workers):
                await w.__aexit__(None, None, None)
        if elsewhere is not None:
            assert elsewhere.stdin is not None
            elsewhere.stdin.close()
            if await elsewhere.wait() != 0:
                raise RuntimeError(f"the child's runner exited with {elsewhere.returncode}")
    finally:
        await env.shutdown()
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")


def package(work: str, name: str, source: str) -> str:
    """A copy of a generated package under a name of its own, so that two can be loaded side by side."""
    shutil.copytree(source, os.path.join(work, name))
    return name


if __name__ == "__main__":
    work = tempfile.mkdtemp(prefix="dandori-children-")
    sys.path.insert(0, work)
    try:
        if sys.argv[1] == "--serve":
            asyncio.run(serve(package(work, "child", sys.argv[2]), sys.argv[3]))
        else:
            parent_dir, child_dir, runs_file, out_file = sys.argv[1:5]
            child_name = None if "DANDORI_CHILD_BY" in os.environ else package(work, "child", child_dir)
            asyncio.run(main(package(work, "parent", parent_dir), child_name, runs_file, out_file))
    finally:
        shutil.rmtree(work, ignore_errors=True)
