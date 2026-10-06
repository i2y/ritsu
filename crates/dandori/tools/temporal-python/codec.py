"""Runs a workflow whose history is encrypted (`history encrypted`, dandori's DESIGN 1.18) on the
Temporal CLI's dev server, through the worker.py and client.py dandori generated for it, with a
codec that writes every payload as base64 under an encoding of its own. Then it reads the history
the server keeps with a client that has no codec, and says what it finds there: the encoding of the
workflow's input, whether a value of the input shows in the clear anywhere in the history, and the
message of the failure the second run ends with. The same as tools/temporal/codec.mjs.

    tools/temporal-python/.venv/bin/python tools/temporal-python/codec.py <generated package> <results.json>

The workflow is tests/encrypted/pay.flow: its one task, `pay`, is the user's, and fails with
`refused` for an account whose number starts with 5500. Two runs: one that pays, one that is refused.
"""

from __future__ import annotations

import asyncio
import base64
import importlib
import json
import os
import shutil
import sys
import tempfile
from typing import Any, Sequence

from temporalio.api.common.v1 import Payload
from temporalio.client import Client, WorkflowFailureError
from temporalio.converter import PayloadCodec
from temporalio.exceptions import ApplicationError
from temporalio.testing import WorkflowEnvironment

# The encoding this codec writes; the encoding a payload had is kept beside it.
ENCODING = b"binary/dandori-test-base64"


class Base64Codec(PayloadCodec):
    async def encode(self, payloads: Sequence[Payload]) -> list[Payload]:
        return [Payload(metadata={"encoding": ENCODING, "dandori-encoding": p.metadata.get("encoding", b"")}, data=base64.b64encode(p.data)) for p in payloads]

    async def decode(self, payloads: Sequence[Payload]) -> list[Payload]:
        out = []
        for p in payloads:
            if p.metadata.get("encoding") != ENCODING:
                out.append(p)
                continue
            out.append(Payload(metadata={"encoding": p.metadata["dandori-encoding"]}, data=base64.b64decode(p.data)))
        return out


class Own:
    async def pay(self, args: dict[str, Any]) -> str:
        account = args["account"]
        if account["number"].startswith("5500"):
            raise ApplicationError("the account is closed", type="refused", non_retryable=True)
        return f"receipt for {account['id']}"


async def run(package: str, out_file: str) -> None:
    client_mod = importlib.import_module(f"{package}.client")
    worker_mod = importlib.import_module(f"{package}.worker")
    codec = Base64Codec()
    results: dict[str, Any] = {}
    async with await WorkflowEnvironment.start_local(ui=False, dev_server_log_level="error") as env:
        target = env.client.service_client.config.target_host
        client = await client_mod.connect(target, codec)
        plain = await Client.connect(target)
        async with worker_mod.make_worker(client, Own()):
            for id, number in [("paid", "4111-1111-1111-1111"), ("refused", "5500-0000-0000-0004")]:
                handle = await client_mod.start(client, id, {"account": {"id": f"acct-{id}", "number": number}})
                end: dict[str, Any]
                try:
                    end = {"succeed": await handle.result()}
                except WorkflowFailureError as e:
                    cause = e.cause
                    end = {"fail": {"error": getattr(cause, "type", None), "cause": getattr(cause, "message", None)}}
                # the history as the server keeps it, read with no codec
                history = await plain.get_workflow_handle(id).fetch_history()
                started = history.events[0].workflow_execution_started_event_attributes
                last = history.events[-1]
                failed = last.workflow_execution_failed_event_attributes if last.HasField("workflow_execution_failed_event_attributes") else None
                kept = history.to_json()
                results[id] = {
                    "end": end,
                    "input_encoding": started.input.payloads[0].metadata["encoding"].decode(),
                    "number_in_the_clear": number in kept,
                    "failure_message": failed.failure.message if failed else None,
                    "failure_encoded": bool(failed and failed.failure.HasField("encoded_attributes")),
                }
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
        f.write("\n")


def main() -> None:
    generated, out_file = sys.argv[1], sys.argv[2]
    work = tempfile.mkdtemp(prefix="dandori-codec-")
    try:
        package = os.path.basename(os.path.normpath(generated))
        shutil.copytree(generated, os.path.join(work, package))
        sys.path.insert(0, work)
        asyncio.run(run(package, out_file))
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
