# Runs the agent call of the default Transport that dandori writes for Python (io.py), so that
# nothing leaves the machine: OpenAI's agents with OpenAI's Agents SDK and a scripted model in
# place of OpenAI's, Claude's with Anthropic's SDK and a stand-in of the Messages API on
# 127.0.0.1. A case with a `status` has the stand-in answer every request with that error
# status, to see that the SDK's client does not retry by itself; an OpenAI agent's then goes to a
# stand-in of the Responses API, through the client the Transport makes (OPENAI_BASE_URL). An agent
# on another server of Open Responses (a call with "url") goes to a stand-in of that server on
# 127.0.0.1, at the same path; a case that says "live" goes to the server the call names, as it
# is. The Python twin of check.mjs: the same cases, and the results in the same shape.
#
#   .venv/bin/python check.py <io.py> <cases.json> <results.json>
#
# The venv:
#   uv venv --python 3.13 tools/agents/.venv
#   uv pip install --python tools/agents/.venv/bin/python -r tools/agents/requirements.txt

from __future__ import annotations

import asyncio
import http.server
import importlib.util
import json
import os
import sys
import threading
import urllib.parse
from typing import Any

from agents import Model, ModelProvider, RunConfig
from agents.testing import ScriptedModel, assistant_message
from openai.types.responses import ResponseOutputMessage, ResponseOutputRefusal


class Provider(ModelProvider):
    """Gives the scripted model for any name, and writes the names down."""

    def __init__(self, model: Model) -> None:
        self.model = model
        self.names: list[str | None] = []

    def get_model(self, model_name: str | None) -> Model:
        self.names.append(model_name)
        return self.model


def message(model: str, text: str, stop: str) -> dict[str, Any]:
    """The Messages API's answer: the text, and how the turn ended; a refusal has no content, as the API reference shows it."""
    refused = stop == "refusal"
    return {
        "id": "msg_test",
        "type": "message",
        "role": "assistant",
        "model": model,
        "content": [] if refused else [{"type": "text", "text": text}],
        "stop_reason": stop,
        "stop_sequence": None,
        "stop_details": {"type": "refusal", "category": None, "explanation": None} if refused else None,
        "usage": {"input_tokens": 10, "output_tokens": 10},
    }


class StandIn(http.server.ThreadingHTTPServer):
    """A stand-in of the Messages API that answers every request with `reply` and writes it down."""

    def __init__(self, reply: tuple[int, Any]) -> None:
        self.reply = reply
        self.asked: list[dict[str, Any]] = []

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(handler) -> None:  # noqa: N805 - the server is `self` here
                raw = handler.rfile.read(int(handler.headers.get("content-length") or 0))
                self.asked.append({"method": "POST", "path": handler.path, "version": handler.headers.get("anthropic-version"), "body": json.loads(raw) if raw else None})
                status, body = self.reply
                data = json.dumps(body).encode()
                handler.send_response(status)
                handler.send_header("content-type", "application/json")
                handler.send_header("content-length", str(len(data)))
                handler.end_headers()
                handler.wfile.write(data)

            def log_message(handler, *args: Any) -> None:  # noqa: N805
                pass

        super().__init__(("127.0.0.1", 0), Handler)


async def openai_failing(io: Any, c: dict[str, Any]) -> dict[str, Any]:
    """An OpenAI agent against a stand-in of the Responses API that answers with an error status."""
    server = StandIn((c["status"], {"error": {"message": "scripted", "type": "server_error"}}))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    os.environ["OPENAI_BASE_URL"] = f"http://127.0.0.1:{server.server_address[1]}/v1"
    os.environ["OPENAI_API_KEY"] = "test"
    os.environ["OPENAI_AGENTS_DISABLE_TRACING"] = "1"
    out: dict[str, Any] = {}
    try:
        try:
            out["answer"] = await io.transport().agent(c["call"])
        except Exception as e:  # noqa: BLE001 - the error's name is what is compared
            out["error"] = type(e).__name__
    finally:
        server.shutdown()
        server.server_close()
    out["asked"] = [{"method": a["method"], "path": a["path"]} for a in server.asked]
    return out


def response(model: str, content: dict[str, Any]) -> dict[str, Any]:
    """The Responses API's answer, as a server of Open Responses gives it: a message whose content is `content`."""
    return {
        "id": "resp_test",
        "object": "response",
        "status": "completed",
        "model": model,
        "output": [{"id": "msg_test", "type": "message", "role": "assistant", "status": "completed", "content": [content]}],
    }


async def open_responses(io: Any, c: dict[str, Any]) -> dict[str, Any]:
    """An agent on a server of Open Responses: a stand-in of it at the call's path, or, `live`, the server itself."""
    transport = io.transport()

    async def run(call: dict[str, Any]) -> dict[str, Any]:
        try:
            return {"answer": await transport.agent(call)}
        except Exception as e:  # noqa: BLE001 - the error's name is what is compared
            return {"error": type(e).__name__}

    call = c["call"]
    if c.get("live"):
        return await run(call)
    if "status" in c:
        reply: tuple[int, Any] = (c["status"], {"error": {"message": "scripted", "type": "server_error"}})
    elif "refusal" in c:
        reply = (200, response(call["model"], {"type": "refusal", "refusal": c["refusal"]}))
    else:
        reply = (200, response(call["model"], {"type": "output_text", "text": c["text"], "annotations": []}))
    server = StandIn(reply)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        out = await run({**call, "url": f"http://127.0.0.1:{server.server_address[1]}" + urllib.parse.urlsplit(call["url"]).path})
    finally:
        server.shutdown()
        server.server_close()
    out["asked"] = [{"method": a["method"], "path": a["path"], "body": a["body"]} for a in server.asked]
    return out


async def claude(io: Any, c: dict[str, Any]) -> dict[str, Any]:
    call = c["call"]
    if "status" in c:
        reply: tuple[int, Any] = (c["status"], {"type": "error", "error": {"type": "api_error", "message": "scripted"}})
    elif "refusal" in c:
        reply = (200, message(call["model"], c["refusal"], "refusal"))
    else:
        reply = (200, message(call["model"], c["text"], "end_turn"))
    server = StandIn(reply)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    out: dict[str, Any] = {}
    try:
        transport = io.transport(claude={"base_url": f"http://127.0.0.1:{server.server_address[1]}", "api_key": "test"})
        try:
            out["answer"] = await transport.agent(call)
        except Exception as e:  # noqa: BLE001 - the error's name is what is compared
            out["error"] = type(e).__name__
    finally:
        server.shutdown()
        server.server_close()
    out["asked"] = server.asked
    return out


def load(path: str) -> Any:
    spec = importlib.util.spec_from_file_location("dandori_io", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


async def main() -> None:
    io_file, cases_file, out_file = sys.argv[1:4]
    io = load(io_file)
    with open(cases_file, encoding="utf-8") as f:
        cases = json.load(f)
    results = []
    for c in cases:
        if c["call"].get("url") is not None:
            results.append(await open_responses(io, c))
            continue
        if c["call"].get("provider") == "claude":
            results.append(await claude(io, c))
            continue
        if "status" in c:
            results.append(await openai_failing(io, c))
            continue
        if "refusal" in c:
            refusal = ResponseOutputRefusal(type="refusal", refusal=c["refusal"])
            message: Any = ResponseOutputMessage(id="scripted-message", type="message", role="assistant", status="completed", content=[refusal])
        else:
            message = assistant_message(c["text"])
        model = ScriptedModel([[message]])
        provider = Provider(model)
        transport = io.transport(agents=RunConfig(model_provider=provider, tracing_disabled=True))
        out: dict[str, Any] = {}
        try:
            out["answer"] = await transport.agent(c["call"])
        except Exception as e:  # noqa: BLE001 - the error's name is what is compared
            out["error"] = type(e).__name__
        if model.calls:
            call = model.calls[0]
            schema = call.output_schema
            out["asked"] = {
                "models": provider.names,
                "calls": len(model.calls),
                "instructions": call.system_instructions,
                "input": call.input,
                "outputType": None if schema is None else {"type": "json_schema", "name": schema.name(), "strict": schema.is_strict_json_schema(), "schema": schema.json_schema()},
                "modelSettings": {k: v for k, v in call.model_settings.to_json_dict().items() if v is not None},
                "tools": [t.name for t in call.tools],
            }
        results.append(out)
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(results, f, ensure_ascii=False, indent=2)
        f.write("\n")


if __name__ == "__main__":
    asyncio.run(main())
