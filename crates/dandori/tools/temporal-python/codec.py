"""Runs a workflow whose history is encrypted (`history encrypted`, dandori's DESIGN 1.18) on the
Temporal CLI's dev server, through the worker.py and client.py dandori generated for it, with a
codec that writes every payload as base64 under an encoding of its own. Then it reads the history
the server keeps with a client that has no codec, and says what it finds there: the encoding of the
workflow's input, whether a value of the input shows in the clear anywhere in the history, the
message of the failure the second run ends with, and the update that answered the callback: its id,
which the history keeps in the clear, and its outcome, the SHA-256 of the answer the workflow took
(dandori's DESIGN 25), as the codec wrote it. The same as tools/temporal/codec.mjs.

    tools/temporal-python/.venv/bin/python tools/temporal-python/codec.py <generated package> <results.json>

The workflow is tests/encrypted/pay.flow. Its task `approve` hands on the id of a callback, which
this runner answers before the task returns, and then twice more, as a caller sends it when the
reply was lost: the same answer, which the workflow took, and another, which the client tells apart
(SentWithAnotherValueError). Its task `pay` is the user's, and fails with `refused` for an account
whose number starts with 5500. Two runs: one that pays, one that is refused.
"""

from __future__ import annotations

import asyncio
import base64
import hashlib
import importlib
import json
import os
import re
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
    """The user's tasks. The approval is answered before the task returns, and then twice more, as
    a caller sends it when the reply was lost: the same answer, and another."""

    def __init__(self, client_mod: Any) -> None:
        self.client_mod = client_mod
        self.client: Any = None
        # by the id of each run: the id of its callback, and how the workflow met each answer to it
        self.answered: dict[str, dict[str, Any]] = {}

    async def outcome(self, callback_id: str, a: dict[str, Any]) -> str:
        """How the workflow met an answer: "taken", or the error the client gave."""
        try:
            await self.client_mod.answer(self.client, callback_id, a)
            return "taken"
        except self.client_mod.SentWithAnotherValueError:
            return "SentWithAnotherValueError"
        except Exception as e:  # said in the results
            return f"{type(e).__name__}: {e}"

    async def approve(self, args: dict[str, Any]) -> None:
        callback_id = args["callback_id"]
        workflow_id = json.loads(callback_id)[0]
        a = {"ok": f"approver of {workflow_id}"}
        first = await self.outcome(callback_id, a)
        again = await self.outcome(callback_id, a)
        another = await self.outcome(callback_id, {"ok": "someone else"})
        self.answered[workflow_id] = {"callback_id": callback_id, "answers": {"first": first, "again": again, "another": another}}

    async def pay(self, args: dict[str, Any]) -> str:
        account = args["account"]
        if account["number"].startswith("5500"):
            raise ApplicationError("the account is closed", type="refused", non_retryable=True)
        return f"receipt for {account['id']}"


def named(update_id: str, callback_id: str) -> str:
    """An update's id as the history keeps it, its hex digits named when they are the SHA-256 of the callback's id."""
    sha256 = hashlib.sha256(callback_id.encode()).hexdigest()[:32]
    return "dandori.answer:<the SHA-256 of the callback's id>" if update_id == f"dandori.answer:{sha256}" else update_id


async def holds_value_sha256(codec: PayloadCodec, p: Payload) -> bool:
    """Whether the payload, read with the codec, is an outcome that holds the SHA-256 of a value."""
    try:
        [plain] = await codec.decode([p])
        took = json.loads(plain.data)
        return isinstance(took, dict) and re.fullmatch(r"[0-9a-f]{32}", str(took.get("value_sha256", ""))) is not None
    except Exception:
        return False


async def run(package: str, out_file: str) -> None:
    client_mod = importlib.import_module(f"{package}.client")
    worker_mod = importlib.import_module(f"{package}.worker")
    codec = Base64Codec()
    own = Own(client_mod)
    results: dict[str, Any] = {}
    async with await WorkflowEnvironment.start_local(ui=False, dev_server_log_level="error") as env:
        target = env.client.service_client.config.target_host
        client = await client_mod.connect(target, codec)
        own.client = client
        plain = await Client.connect(target)
        async with worker_mod.make_worker(client, own):
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
                accepted = [e.workflow_execution_update_accepted_event_attributes for e in history.events if e.HasField("workflow_execution_update_accepted_event_attributes")]
                completed = [e.workflow_execution_update_completed_event_attributes for e in history.events if e.HasField("workflow_execution_update_completed_event_attributes")]
                took = completed[0].outcome.success.payloads[0] if completed and completed[0].outcome.HasField("success") and completed[0].outcome.success.payloads else None
                approved = own.answered.get(id, {})
                results[id] = {
                    "end": end,
                    "input_encoding": started.input.payloads[0].metadata["encoding"].decode(),
                    "number_in_the_clear": number in kept,
                    "failure_message": failed.failure.message if failed else None,
                    "failure_encoded": bool(failed and failed.failure.HasField("encoded_attributes")),
                    "answers": approved.get("answers"),
                    "update_ids": [named(a.accepted_request.meta.update_id, approved.get("callback_id", "")) for a in accepted],
                    "update_result_encoding": took.metadata["encoding"].decode() if took else None,
                    "update_result_in_the_clear": (b"value_sha256" in took.data) if took else None,
                    "update_result_decodes_to_value_sha256": (await holds_value_sha256(codec, took)) if took else None,
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
