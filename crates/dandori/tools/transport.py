# A Transport for the Python dandori writes (for Temporal and for pydantic-graph), in place of
# Lambda, HTTP and the AWS APIs: every call goes to `run.take(call, callback_id)` as Step
# Functions would send it, which writes it down and gives the scenario's answer. The Python
# twin of transport.mjs, shared by temporal-python/run.py and pydantic-graph/run.py.
#
# spec["http"]: [ { method, url, errors: { <error>: <status> } } ]  (url with {placeholders})
# spec["aws"]:  [ { api: "<service>:<action>", errors: { <error>: <exception> }, keyParam } ]
#
# A callback task's submit hands on `callback_id` (in the Lambda payload, or in the SQS
# message); the call is written down without it, `take` gets the id, and
# `run.answer_later(id, answer)` is called (and awaited, when it gives something to await) with
# the answer the scenario gives the callback.

from __future__ import annotations

import inspect
import re
from typing import Any


def make_transport(spec: dict[str, Any], run: Any) -> Any:
    http_tasks = []
    for t in spec.get("http", []):
        parts = re.split(r"\{[^}]*\}", t["url"])
        http_tasks.append({**t, "re": re.compile("^" + "[^/?]*".join(re.escape(p) for p in parts) + "$")})
    aws_tasks = spec.get("aws", [])

    def error_name(kind: str) -> str:
        return "Dandori.Test.Failure" if kind == "failure" else kind

    class StandIn:
        async def lambda_(self, fn: str, payload: dict[str, Any]) -> dict[str, Any]:
            rest = {k: v for k, v in payload.items() if k != "callback_id"}
            callback_id = payload.get("callback_id")
            ans = run.take({"lambda": fn, "payload": rest}, callback_id)
            if callback_id is not None:
                answered = run.answer_later(callback_id, ans)
                if inspect.isawaitable(answered):
                    await answered
                return {"ok": None}
            if "ok" in ans:
                return {"ok": ans["ok"]}
            return {"error": error_name(ans["error"]), "message": "scripted"}

        async def http(self, req: dict[str, Any]) -> dict[str, Any]:
            ans = run.take({k: v for k, v in req.items() if k != "form"}, None)
            if "ok" in ans:
                return {"status": 200, "body": ans["ok"]}
            task = next((t for t in http_tasks if t["method"] == req["http"] and t["re"].match(req["url"])), None)
            status = (task or {}).get("errors", {}).get(ans["error"], 500)
            return {"status": status, "body": "scripted"}

        async def aws(self, service: str, action: str, input: dict[str, Any]) -> dict[str, Any]:
            api = f"{service}:{action}"
            args = input
            callback_id = None
            body = input.get("MessageBody")
            if api == "sqs:sendMessage" and isinstance(body, dict) and "callback_id" in body:
                callback_id = body["callback_id"]
                args = {**input, "MessageBody": {k: v for k, v in body.items() if k != "callback_id"}}
            ans = run.take({"aws": api, "args": args}, callback_id)
            if callback_id is not None:
                answered = run.answer_later(callback_id, ans)
                if inspect.isawaitable(answered):
                    await answered
                return {"ok": {"MessageId": "message-1"}}
            if "ok" in ans:
                return {"ok": ans["ok"]}
            task = next((t for t in aws_tasks if t["api"] == api), None)
            return {"error": (task or {}).get("errors", {}).get(ans["error"], error_name(ans["error"])), "message": "scripted"}

    return StandIn()
